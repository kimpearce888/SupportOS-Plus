# rule.md — mandatory workflow rules

These rules govern how the plan (`plan.md`) is executed. They are mandatory and must not be bypassed.

Statuses: an item moves `plan.md` → `progress.md` → implementation → verification → `completed.md`.

## Rule 1 — Five items at a time

Fix **exactly five items at a time** from `plan.md`.

Never work on more than five plan items simultaneously.

## Rule 2 — Start of work

Before starting an item:

1. Select the next five items from `plan.md`.
2. Remove those five items from `plan.md`.
3. Add those five items to `progress.md`.
4. Begin implementation only after the items have been moved to `progress.md`.

## Rule 3 — During work

Only work on the items currently recorded in `progress.md`.

Do not start another item until the current five items have been completed and verified.

## Rule 4 — Completion

After the current five items are fixed and verified:

1. Remove those items from `progress.md`.
2. Add those items to `completed.md`.
3. Record the completed work and verification result.
4. Stop and report the result to the user.

## Rule 5 — Verification

An item is complete only after the relevant code, tests, builds, or other required checks have been successfully verified.

Do not mark an item as completed based only on code changes.

## Rule 6 — No skipped items

Every item originally identified in `plan.md` must eventually be processed.

Do not skip, merge, or silently discard items.

## Rule 7 — Keep files synchronized

At all times:

* `plan.md` = remaining TODO items
* `progress.md` = exactly five active items
* `completed.md` = completed and verified items

The same item must never exist in more than one of these files at the same time.

## Rule 8 — Do not automatically start the next items

After completing the current five items, **do not begin the next items automatically**.

Instead:

1. Finish and verify the current five items.
2. Update `progress.md` and `completed.md`.
3. Give the user a clear summary of the completed work and verification result.
4. Tell the user to say **`continue`** to proceed to the next five items in `plan.md`.

When the user says **`continue`**, resume the workflow by selecting the next five items from `plan.md`.

## Rule 9 — Blocked work

If any of the current items cannot be completed because of a blocker:

* Keep the blocked item in `progress.md`.
* Clearly document the blocker.
* Do not start another item.
* Report the blocker to the user.
* Tell the user what is required to continue.

## Execution workflow

Follow this state transition exactly:

`plan.md → progress.md → implementation → verification → completed.md → STOP`

After stopping, wait for the user to say:

`continue`

Then process the next five items.
