---
name: session-resume
description: Resume interrupted Bill Analyser tasks after network issues or missing retry buttons by restoring context from .git/ai/last-session.md, git status, git diff, and repository rules.
---

# Session Resume

Use this shared workflow when a task was interrupted by network issues, editor restart, window closure, or a missing retry button.

## Recovery order

1. Read `AGENTS.md` for canonical repository rules.
2. Read `docs/PROJECT_OVERVIEW.md` for current architecture and domain boundaries.
3. If it exists, read `.git/ai/last-session.md` for the latest session snapshot.
4. Inspect current `git status` and `git diff`.
5. Compare the snapshot against the current branch and HEAD before resuming edits.

## Recovery goals

- reconstruct the most recent task focus
- identify files that were being edited
- recover the next verification step
- detect when the snapshot is stale because branch or HEAD changed

## Expected output

When this workflow is invoked, produce a concise recovery summary containing:

- current branch and HEAD
- whether `.git/ai/last-session.md` matches the current workspace state
- files that were recently edited
- the next recommended verification step
- the next recommended implementation step

## Safety rules

- Treat `.git/ai/last-session.md` as metadata only.
- Never store or echo secrets, environment variables, tokens, or full diff contents into the snapshot.
- If the snapshot conflicts with the current `git status` / `git diff`, call out the mismatch explicitly before resuming work.
- If the snapshot does not exist yet, fall back to `git status` / `git diff` and reconstruct the next step from the current workspace state.