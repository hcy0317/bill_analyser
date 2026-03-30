---
name: approved-plan-execution
description: Execute an already approved Bill Analyser plan or checklist in small verified increments. Use after `/plan`, after an explicit user-approved checklist, or when resuming a vetted implementation plan; refreshes `.git/ai/task-state.json` before execution.
---

# Approved Plan Execution

Use this workflow when planning is done and implementation should begin immediately.

## Preconditions

- There is an approved plan, checklist, or explicit user instruction to proceed.
- The goal is clear enough to implement without reopening broad planning.

## Execution workflow

1. Read `AGENTS.md`.
2. Read `docs/PROJECT_OVERVIEW.md` if runtime/business flows are involved.
3. Read the approved plan or checklist.
4. Read `.git/ai/task-state.json` and `.git/ai/last-session.md` if they exist.
5. Restate the execution goal and constraints briefly.
6. Refresh `.git/ai/task-state.json` before editing.
   - Preferred command: `python scripts/hooks/task_state.py --trigger approved-plan --status in_progress --title "<execution goal>"`
7. Convert the plan into the smallest concrete implementation slice.
8. Implement that slice.
9. Run path-sensitive verification before moving to the next slice.
10. If you pause before finishing, switch to the `session-handoff` workflow.

## Guardrails

- Do not reopen full planning unless a critical ambiguity blocks execution.
- Keep changes small and verifiable.
- Update tests before risky behavior changes whenever feasible.
- If the approved plan is stale relative to `git diff`, call it out before continuing.

## Expected result

- Execution begins from an approved plan instead of re-planning.
- `.git/ai/task-state.json` captures the active work item.
- Verification remains coupled to each implementation slice.
