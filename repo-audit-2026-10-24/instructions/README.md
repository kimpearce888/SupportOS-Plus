# Repository Audit Instruction Package

This package defines a source-code-led, end-to-end repository audit with real application testing, evidence collection, incremental reporting, and session-to-session continuity.

## Files and reading order

1. `00-BOOTSTRAP.md` — mandatory reading gate and launch instructions.
2. `01-MISSION-SOURCE-OF-TRUTH-AND-INVENTORY.md` — audit mission, evidence hierarchy, architecture discovery, exhaustive functionality inventory, and partial implementations.
3. `02-BUILD-EXECUTION-AND-MANUAL-QA.md` — environment discovery, build/install, human-style UI testing, workflows, edge cases, and safe execution.
4. `03-EVIDENCE-AND-EXCEPTIONS.md` — exact screenshot/video rules, allowed exceptions, alternative evidence, manifests, and coverage calculations.
5. `04-FINDINGS-REPORTING-AND-COMPLETION.md` — defects and fixes, coverage reconciliation, report structure, and completion gate.
6. `05-HANDOVER-CHECKPOINTS-AND-GIT.md` — interim reporting, durable checkpoints, handover, fresh-session resume, and safe Git publication.
7. `06-REQUIREMENT-TRACEABILITY.csv` — one row per distinct mandatory obligation, with stable IDs and verification guidance.

## How to use

Give the agent `00-BOOTSTRAP.md` as the entry-point instruction. It must read every listed file fully and reconcile the traceability checklist before it audits the application. The repository URL should be supplied to the agent separately or placed in the bootstrap file's input field.

The numbered documents are the complete instruction set; they are not optional reference reading. If a file cannot be read completely, the agent must record that limitation and must not claim the reading gate passed.

## Design principles

- The actual implementation and observed behavior are the primary source of truth.
- Existing tests are supplementary evidence, not proof of product correctness.
- The audit must test the installed/running application through real interfaces where feasible.
- Every executed manual UI step requires a validated screenshot or a narrowly justified, recorded exception.
- Work is reported and saved incrementally, not only at the end.
- A new agent/session must resume from the durable handover rather than starting over.
- The audit may identify and recommend fixes, but must not modify application code unless explicitly authorized.
- Never claim a test, artifact, commit, push, or publication succeeded unless it was actually verified.

## Suggested audit output folder

Create a folder at the repository root, such as `repo-audit-YYYY-MM-DD/`. The required contents and responsibilities are defined in the instruction files. Adapt filenames only when necessary and keep all references stable and relative.
