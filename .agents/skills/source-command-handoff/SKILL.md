---
name: "source-command-handoff"
description: "Create a restart-safe handoff summary, refresh `.git/ai/task-state.json`, and make the next session immediately productive."
---

# source-command-handoff

Use this skill when the user asks to run the migrated source command `handoff`.

## Command Template

# Handoff Command

Use the shared workflow from `.agents/skills/session-handoff/SKILL.md`.

## Required steps

1. Read `AGENTS.md`.
2. Read `.agents/skills/session-handoff/SKILL.md`.
3. Read `.git/ai/last-session.md` if it exists.
4. Inspect `git status` and `git diff`.
5. Refresh task state with `python scripts/hooks/task_state.py --trigger handoff --status handoff --title "<current objective>"` when command execution is available.
6. Produce a concise handoff summary with the required sections from the shared workflow.

## Output rules

- Prefer concrete file paths over generic wording.
- Say exactly what verification already ran.
- Say exactly what verification is still required.
- If there is a blocker, put it in its own bullet.
- If the snapshot/task-state looks stale relative to current diff, call that out explicitly.
