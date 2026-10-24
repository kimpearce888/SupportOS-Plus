# 04 — Findings, Fix Recommendations, Reporting, and Completion

## 1. Classify every finding honestly

Investigate build/install/runtime failures, incorrect logic/state transitions, broken navigation, inaccessible controls, incomplete workflows, validation/persistence/data-loss issues, configuration errors, integration/authentication/permission/timeout/retry failures, race/concurrency/background-task problems, misleading or swallowed errors, performance/resource/scaling problems, security weaknesses, accessibility/usability barriers, malformed/empty/duplicate/large/boundary input behavior, sequence-dependent defects, clean-vs-used environment differences, and missing functionality that prevents a workflow completing.

Use evidence status independently from severity:

- **Confirmed at runtime:** reproduced in the actual program.
- **Confirmed by implementation analysis:** implementation establishes a defect, but runtime reproduction was not achieved.
- **Suspected:** evidence indicates a potential problem, but behavior/root cause is unverified.
- **Blocked/unverified:** required test could not be performed.

Never present assumptions as confirmed defects. Do not mark a feature absent merely because it cannot be accessed. Save meaningful findings promptly; root-cause analysis may continue afterward.

## 2. Mandatory finding format

Each important finding must contain:

- **Finding ID/title:** stable unique identifier and precise description.
- **Classification:** bug, incomplete implementation, functional gap, blocker, build/install defect, integration failure, security concern, performance issue, accessibility/usability issue, or justified alternative.
- **Severity:** Critical/High/Medium/Low with practical impact. Keep separate from confidence.
- **Confidence/status:** confirmed, strongly supported, suspected; say whether cause is confirmed, inferred, or unknown.
- **Affected features/workflows:** IDs and user journey(s).
- **Reproduction steps:** exact starting state, prerequisites, inputs, and action sequence.
- **Expected behavior:** supported by implementation and logical requirements of completing the feature.
- **Actual behavior:** observed result, error, missing effect, or incorrect state.
- **Evidence:** screenshot/video/log references, source paths, function names, and line ranges where available.
- **Root cause:** execution path, logic, state transition, validation, integration boundary, dependency, or design choice responsible; distinguish fact from hypothesis.
- **Recommended fix:** specific code/architectural correction.
- **Reasoning:** why it fixes the cause instead of masking the symptom.
- **Affected components:** related modules, persistence, integrations, and workflows.
- **Regression risks:** likely side effects and tests required.
- **Acceptance criteria:** observable conditions for successful correction.
- **Verification plan:** exact test cases/manual journey and evidence needed to verify the fix.
- **Next action:** if investigation or confirmation remains.

Recommendations must be actionable: identify module/function/state/persistence/validation path wherever possible. Do not propose merely adding a test, suppressing an error, hiding a broken control, weakening validation, or disabling a feature without a justified product/technical reason. Where solutions differ, recommend the best-supported option and explain trade-offs.

If the root cause cannot be established without instrumentation or a change, state what is unknown and what investigation is required. Do not modify product code or implement fixes unless explicitly authorized.

## 3. Coverage reconciliation

Reconcile source inventory with test records continuously and again before finalization. Verify:

- Every discovered feature has an inventory entry.
- Every exposed control and meaningful option has a status.
- Every workflow has an entry point, expected outcome, branches, and recorded coverage.
- Every meaningful branch is tested or marked untested/blocked.
- Every executed UI step has a validated screenshot or specific evidence exception.
- Every exception matches an allowed type, has required alternative evidence, and is reviewed.
- Rejected/unresolved exceptions remain visible as evidence gaps.
- Important defects have reproduction details and evidence.
- Every fix recommendation is tied to a documented problem.
- Every evidence file exists, opens/plays, and supports the stated claim.
- Build/install/launch/shutdown outcomes are recorded.
- Automated/existing test results are separate from manual QA.
- Blocked integrations, inaccessible paths, environmental limitations, and unverified behavior are disclosed.
- Duplicates are consolidated without losing reproduction paths/workflows.
- Evidence percentages use the stated definitions.
- No statement is stronger than evidence permits.

Blocked tests must record attempted steps, actual blocker, evidence, likely cause only if supported, and prerequisites for further verification. Missing screenshots remain explicit gaps until captured or a valid exception is accepted. Exceptions must not convert failed, blocked, or untested operations into passing tests.

Completeness is shown by reconciled inventories, test results, evidence manifest, and exception register—not by simply claiming everything was checked. Reconcile after batches to prevent late discovery of gaps.

## 4. Main report requirements

Maintain `REPORT.md` throughout the audit. It must be marked `IN PROGRESS` until final reconciliation. Write sections as they become supportable; do not wait until the end to begin the report. Use the inventories and registers as detailed sources, and clearly distinguish preliminary from confirmed results.

Required report sections:

1. Scope and methodology; how source of truth was established.
2. Environment and reproducibility: revision, OS, toolchain, build commands, installation, and observed execution.
3. Key findings overview without replacing detailed registers.
4. Program purpose and every distinct problem addressed/attempted.
5. Architecture, components, data flows, entry points, dependencies, state, integrations, and lifecycle.
6. Complete feature/control/option/configuration/operational capability inventory (reference detailed inventory).
7. Distinctive implementation behavior and supported trade-offs.
8. Partial/unreachable/disconnected implementations.
9. Complete workflow map, branches, destinations, outcomes.
10. Manual test results for controls, features, journeys, and edge cases.
11. Defect register with severity, reproduction, behavior, evidence, and root cause.
12. Fix recommendations with rationale, acceptance criteria, and regression checks.
13. Build/install/runtime failures.
14. Existing test-suite results and limitations, separately from manual QA.
15. Evidence index with step/workflow/finding links.
16. Evidence exceptions/gaps, accepted/rejected exceptions, limitations, and next actions.
17. Coverage counts for tested/passed/failed/blocked/unverified items with definitions.
18. Continuity/publication state: audited revision, latest audit checkpoint, branch, handover, remaining work, and remote status.

The report must let another engineer understand the actual application, reproduce major findings, and assess fixes without repeating initial discovery. Do not replace detailed feature/problem/workflow/finding inventories with a summary. Reference source files, functions, line ranges, stable IDs, and relative evidence paths.

## 5. Final completion gate

Check each item and record actual status:

- [ ] Repository structure and build-relevant implementation understood.
- [ ] Architecture, execution paths, and data flows documented.
- [ ] Distinct problems and capabilities inventoried.
- [ ] Every discovered control and meaningful option accounted for.
- [ ] Partial, disconnected, and unreachable functionality documented.
- [ ] Application built, installed, and launched where feasible.
- [ ] Existing tests run where feasible and separately reported.
- [ ] Manual testing used the real UI wherever environment permits.
- [ ] Every discovered workflow executed or marked blocked/unverified.
- [ ] Relevant input/state/error/recovery/cross-feature cases tested.
- [ ] Every executed manual UI step has validated screenshot or exact exception.
- [ ] Exceptions satisfy a defined category and include required details.
- [ ] Missing/unreadable/invalid evidence is accounted for.
- [ ] Videos captured where valuable/warranted and validated when produced.
- [ ] Findings include reproduction, evidence, and justified fix recommendations.
- [ ] Coverage reconciled to complete inventory.
- [ ] Evidence manifest reconciled to files on disk.
- [ ] Screenshot coverage, exceptions, and gaps reported separately.
- [ ] Main report and supporting files exist.
- [ ] `HANDOVER.md` states final verified state and any remaining work.
- [ ] Progress and session records agree with the final report.
- [ ] Only intended audit artifacts are committed.
- [ ] Commit push verified or exact blocker documented.
- [ ] Branch, commit hash, and publication status recorded.

Blocked, infeasible, and unverified items must not be marked complete; give reason and outstanding action. Never claim absence of all possible bugs or exhaustive testing of every possible input combination.

## 6. Required final user-facing response

Report:

1. Repository URL and exact application revision audited.
2. Audit folder and remote link when available.
3. Remote branch, commit hash, and verified publication status.
4. Actual build/install/launch/manual-testing outcomes.
5. Counts for features, controls, workflows, executed steps, findings, unresolved/blocked items, and definitions of counts (avoid double counting).
6. Executed UI steps requiring screenshots, validated screenshots, accepted exceptions by type, and evidence gaps.
7. Major findings and highest-impact recommended fixes.
8. Important environment/evidence/scope limitations.
9. Handover status, location of resume instructions, and exact next action if work remains.

Do not present build success, passing tests, or a pushed report as proof the program works. Report exactly what was verified, what failed, evidence, exceptions, what has actually been published, and how another agent resumes without repeating completed work.
