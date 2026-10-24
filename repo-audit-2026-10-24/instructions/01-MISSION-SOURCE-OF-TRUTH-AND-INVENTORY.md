# 01 — Mission, Source of Truth, Architecture, and Inventory

## 1. Mission and questions to answer

Audit the actual application implementation and determine:

1. What the program is built to do.
2. Every distinct problem it attempts to solve, including secondary/supporting and operational problems.
3. How it is built, how components connect, how data moves, and how state is stored/restored.
4. Every implemented feature, control, option, configuration, supported operation, and user-visible or operational capability.
5. What the implementation does differently, its practical consequences, trade-offs, and limitations.
6. Which capabilities are partial, disconnected, unfinished, unreachable, or only superficially implemented.
7. Which gaps, bugs, errors, blockers, security weaknesses, performance issues, integration failures, and usability/accessibility problems exist.
8. For each finding, how to reproduce it, what caused it (or what remains unknown), and what should be changed and why.
9. Which capabilities and complete workflows were actually tested, what evidence supports each result, and what remains unverified.

Do not compress distinct problems or features into a high-level summary. The final inventory and explanations must be detailed enough for another qualified engineer to understand the implementation and act on findings without repeating the full initial investigation.

## 2. Primary source-of-truth rules

The actual source code and files that genuinely determine build/runtime behavior are primary sources of truth. Inspect, as applicable:

- Application entry points, source modules, functions, methods, services, handlers, and execution paths.
- Build manifests, dependency declarations, compiler/toolchain settings, feature flags, build scripts, and packaging configuration.
- Runtime configuration, schemas, defaults, environment variables, CLI arguments, supported modes, and validation constraints.
- UI definitions, routes, navigation, components, forms, dialogs, event handlers, state management, and accessibility-related interaction logic.
- Data models, database schemas, migrations required by the application, persistence, transactions, import/export, caching, and recovery.
- APIs, request handlers, network clients, protocols, integration adapters, webhooks, authentication, and authorization.
- Background tasks, scheduling, queues, event systems, IPC, concurrency, and shutdown/startup behavior.
- Templates, embedded assets, resource files, icons, translations, and non-code files consumed by the program.
- Error handling, fallback logic, permission checks, feature gates, installation and upgrade mechanisms, and runtime dependency requirements.

Trace behavior between components. A declaration, interface, function name, or screen is not proof that the capability works.

### Materials that are not authoritative proof of behavior

Do not treat README files, descriptions, roadmaps, specifications, task lists, TODO comments, changelogs, release notes, marketing material, previous agent reports, comments claiming completion, test names, fixtures, mocks, snapshots, assertions, or passing tests as authority for what the program actually does.

Operational instructions may be consulted to locate a build target or command, but verify them against build configuration and observed execution. Existing test suites are supplementary verification: passing tests do not establish correctness or feature completeness, and missing tests do not prove a feature is absent.

If the implementation contradicts documentation, tests, comments, or previous reports, document the contradiction with source and runtime evidence. The code may establish an intended execution path, but runtime testing is needed to establish actual operation wherever feasible.

## 3. Repository discovery and architecture map

Understand the repository before broad functional testing. Identify:

1. Entry points and supported build targets.
2. Languages, frameworks, libraries, dependencies, and runtime requirements.
3. Module boundaries, call relationships, services, and application layers.
4. Data inputs, transformations, outputs, and external interactions.
5. State initialization, mutation, persistence, restoration, and deletion.
6. UI/backend, backend/storage, and service/integration communication.
7. Configuration, authentication, authorization, permissions, feature flags, and validation.
8. Background jobs, events, queues, scheduling, concurrency, and IPC.
9. Build, packaging, install, upgrade, startup, shutdown, and recovery paths.
10. Unused, unreferenced, disconnected, unreachable, stubbed, mocked, placeholder, or incomplete paths.

Trace important operations from their entry point to their effects. Check callers/callees, routing, event handlers, state transitions, data models, validation, persistence, integration boundaries, and error paths. Search for no-op handlers, hardcoded results, unused settings, incomplete branches, unreferenced code, missing persistence, and failures that are swallowed or misreported.

Record the resulting map in `ARCHITECTURE.md`. Update it incrementally as new areas are understood; do not wait until the whole repository is mapped.

## 4. Program purpose and problem inventory

### Purpose

Explain actual purpose based on implemented capabilities, execution paths, data structures, workflows, and integration logic. Identify actual actors/use cases implied by the implementation, tasks supported, and outcomes the program can produce.

### Distinct problems

Identify every distinct problem the implementation addresses or attempts to address. Include primary, secondary, supporting, operational, data-management, and integration problems. Do not merge several distinct problems into one vague entry.

For each problem, record:

- Problem and use case.
- Mechanism implemented to address it.
- Relevant source paths, functions, and execution route.
- Runtime result and limitations.
- Status: working, partial, broken, blocked, unverified, or only inferred.

If an apparent goal cannot be substantiated by implementation, label it inferred/unsupported rather than claiming it is solved.

### Construction

Explain architecture, responsibilities, important code paths, state management, data flow, persistence, dependencies, external interactions, lifecycle, and error handling. Describe important behaviors through execution paths, not just filenames.

### Distinctive behavior

Identify implemented choices or mechanisms that materially change operation. Explain where they are implemented, why they matter, practical advantages/trade-offs, and limitations. Do not invent comparisons or claims of uniqueness without evidence; describe distinctive behavior without unsupported comparative claims.

## 5. Exhaustive feature and functionality inventory

Inventory every discovered capability, not just major features. Include, where applicable:

- Every window, page, screen, view, tab, panel, section, route, and navigation destination.
- Every button, link, icon action, menu, context-menu item, toolbar control, keyboard shortcut, and alternate entry point.
- Every form, field, text area, dropdown, selector, checkbox, radio button, toggle, slider, filter, search, sort, and date control.
- Every configuration setting, default, permitted value, range, constraint, validation rule, dependency, and interaction with other settings.
- Every create/read/update/delete, import/export, search/filter/sort, and bulk operation.
- Every role/permission-dependent feature, login/authentication path, access restriction, and denied path.
- Every API, integration, webhook, protocol, network interaction, background job, scheduled operation, and supported CLI/operating mode.
- Every reporting, analytics, logging, notification, automation, and recovery capability.
- Every loading, empty, initial, success, warning, error, disabled, confirmation, and final state.
- Every persistence, startup, shutdown, restart, restoration, and recovery behavior.
- Any hidden, conditional, partial, disconnected, unreachable, or otherwise non-obvious capability.

Each feature inventory entry must have:

| Field | Required content |
|---|---|
| Feature ID | Stable, unique identifier |
| Name and purpose | What the capability does |
| Entry point | How it is accessed |
| Controls/options | Every implemented action and meaningful alternative |
| Source paths | Files, functions, handlers, endpoints, and line ranges where available |
| Execution path | How the action flows through the application |
| Expected effect | Observable result justified by implementation |
| Dependencies | State, data, permissions, configuration, or services required |
| Test status | Passed, failed, partial, blocked, not attempted, or inconclusive |
| Evidence | Screenshot/video/log/test-step references |

Do not omit small, redundant, conditional, hidden, or secondary controls. Trace declarations to implementation and verify runtime effects wherever feasible. Maintain the inventory as the central source for test planning and final coverage reconciliation.

Continuity rule: add features as discovered. Maintain stable IDs and a last-completed item so a new session can continue without rescanning completed areas merely to rebuild the inventory.

## 6. Partial, unfinished, or disconnected implementation analysis

Look for:

- Controls with missing/incomplete handlers or actions that do nothing.
- Forms that accept values but do not validate, process, or persist them correctly.
- Navigation that terminates prematurely or screens unreachable by normal paths.
- Backend functionality without a usable entry point and UI functionality without backend logic.
- Declared settings/data fields never actually consumed.
- Incomplete CRUD, validation, authorization, loading/error states, and recovery paths.
- Active mocks, placeholders, hardcoded values, simulated outcomes, and stubs.
- Features that work once but fail on repetition or restart.
- Apparent success without the required state/data change.
- Features that work alone but fail within connected journeys.

For each capability, state what exists, what is missing, where execution stops, what prevents completion, the user impact, and what a complete implementation would require. Distinguish intentional non-support from unfinished implementation; do not infer abandonment solely from dates, names, or comments.

Save findings as soon as evidence supports them. If root cause is not yet confirmed, record a preliminary/partially investigated finding with its confidence, evidence, and next investigative action.
