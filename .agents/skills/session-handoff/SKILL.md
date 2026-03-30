---
name: session-handoff
description: Create a restart-safe handoff for partially completed Bill Analyser work. Use when ending a session with open diff, pausing mid-task, or handing unfinished implementation to a future session; refreshes `.git/ai/last-session.md` and `.git/ai/task-state.json`.
---

# Session Handoff

Use this workflow when you want the next session to resume without guesswork.

## Inputs to gather first

1. Read `AGENTS.md`.
2. Read `docs/PROJECT_OVERVIEW.md` if the task touches business/runtime flows.
3. Read `.git/ai/last-session.md` if it exists.
4. Inspect `git status` and `git diff`.
5. Identify the main unfinished objective plus the next verification step.

## Handoff workflow

1. Confirm the current objective in one sentence.
2. Summarize what is already done.
3. List what is still pending.
4. Call out blockers or assumptions explicitly.
5. Refresh `.git/ai/task-state.json` with the current objective and status.
   - Preferred command: `python scripts/hooks/task_state.py --trigger handoff --status handoff --title "<current objective>"`
6. Ensure `.git/ai/last-session.md` is up to date.
7. Produce a concise handoff summary in chat.

## Required handoff sections

- Current objective
- Completed work
- Remaining work
- Verification already done
- Verification still required
- Next implementation step
- Next verification step
- Blockers / assumptions

## Safety rules

- Never dump full patch contents into the handoff.
- Never store secrets, tokens, or environment variables in `.git/ai/*`.
- If `git diff` and `.git/ai/last-session.md` disagree, call out the mismatch explicitly.
- Prefer concrete file paths and commands over vague prose.

## Expected result

After this workflow, the next session should be able to:

- read `.git/ai/last-session.md`
- read `.git/ai/task-state.json`
- inspect `git status` / `git diff`
- continue implementation without re-discovering the task from scratch
