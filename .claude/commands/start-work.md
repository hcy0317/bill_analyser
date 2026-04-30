---
description: Execute from an approved plan or checklist, refresh `.git/ai/task-state.json`, and move directly into implementation with verification.
---

# Start Work Command

Use the shared workflow from `.agents/skills/approved-plan-execution/SKILL.md`.

## Required steps

1. Read `AGENTS.md`.
2. Read `.agents/skills/approved-plan-execution/SKILL.md`.
3. Read the approved plan, checklist, or user-approved execution notes.
4. Read `.git/ai/task-state.json` and `.git/ai/last-session.md` if they exist.
5. Restate the execution goal in 1-2 sentences.
6. Refresh task state with `python scripts/hooks/task_state.py --trigger approved-plan --status in_progress --title "<execution goal>"` when command execution is available.
7. Break the approved plan into the smallest concrete execution slice and start implementing immediately.
8. Run path-sensitive verification before moving to the next slice.
9. If the approved plan is missing or stale relative to current diff, call it out before editing.

## Output rules

- Do not re-plan from scratch unless a critical ambiguity blocks implementation.
- Keep the first response short: execution goal, first slice, first verification step.
- If you must stop mid-way, switch to `/handoff` instead of ending with an implicit state.
