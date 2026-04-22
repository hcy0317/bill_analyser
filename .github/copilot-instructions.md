# Bill Analyser Copilot / VS Code Adapter

Shared repository rules live in [AGENTS.md](../AGENTS.md).
This file is the Copilot / VS Code discovery adapter for this repository and should remain thinner than `AGENTS.md`.

## Start here

1. Read [AGENTS.md](../AGENTS.md) for canonical shared repository rules.
2. Use this file only for Copilot / VS Code-specific discovery and runtime notes.
3. Keep cross-tool guidance in `AGENTS.md` or shared skills instead of duplicating it here.

## What Copilot should discover in this repository

- Canonical shared rules: `AGENTS.md`
- Copilot / VS Code adapter: `.github/copilot-instructions.md`
- File-scoped instructions: `.github/instructions/**/*.instructions.md`
- Repo-specific Copilot agent metadata: `.github/agents/openai.yaml`
- Prompt library: `.github/prompts/*.prompt.md`
- Copilot-native hooks: `.github/hooks/*.json`
- Copilot workflows and health checks: `.github/workflows/*`
- Shared cross-tool skills: `.agents/skills/`
- Repo-specific Copilot-local skills: `.github/skills/`

## Copilot-specific guidance

- VS Code also discovers `AGENTS.md`, `CLAUDE.md`, `.claude/rules/`, `.claude/agents/`, `.agents/skills/`, and project-level Claude hook wiring from `.claude/settings.json`.
- Prefer `AGENTS.md` for repository-wide behavior, architecture boundaries, and workflow policy.
- Prefer `.github/instructions/**/*.instructions.md` only when Copilot needs `applyTo`-style scoping or file-family-specific guidance.
- Prefer `.agents/skills/` for reusable cross-tool workflows; use `.github/skills/` only for Copilot-only or migration-stage skills.
- Prefer `.github/prompts/` for repeatable task entrypoints rather than growing this adapter or `AGENTS.md` into prompt catalogs.
- Generic agents / skills / prompts belong at user level; keep repo copies only when they encode Bill Analyser-specific delta.
- Keep this adapter focused on discovery, runtime wiring, and Copilot-specific deltas; if it starts reading like a second `AGENTS.md`, it has eaten too much spinach.

## Current Bill Analyser Copilot surfaces

- Modular workspace instructions: `.github/instructions/ecc/`
- Copilot agent metadata: `.github/agents/openai.yaml`
- Copilot prompts: `.github/prompts/`
- Copilot-local skills: `.github/skills/`
- Shared repository skills: `.agents/skills/`
- Shared interruption-resume skill: `.agents/skills/session-resume/`
- Copilot hooks: `.github/hooks/`
- Copilot workflow health check: `.github/workflows/agent-stack-health.yml`

## Hook baseline

- Copilot repo guard is declared in `.github/hooks/repo-guard.json`.
- The shared guard implementation lives in `scripts/hooks/pre_tool_repo_guard.py`.
- Copilot native `preToolUse` / `postToolUse` / `stop` hooks also bridge to `scripts/hooks/copilot_global_hook_bridge.py`, which can dispatch optional user-level hooks from `~/.copilot/hooks/` when that global layer exists.
- Project-level Claude settings in `.claude/settings.json` point at the same repo guard so Copilot and Claude stay aligned.
- Commit / upload candidate discovery must stay git-aware: respect `.gitignore`, `.git/info/exclude`, and `core.excludesFile`; use `git check-ignore -v -- <path>` when auditing a disputed path, and `git ls-files -- <path>` when you need to confirm it is already tracked.
- Post-edit and stop-session reminders live under `.github/hooks/` and should stay thin, deterministic, and repository-specific.
- Use `/hooks` when you want a visible diagnostic entrypoint that explains which hooks are active, bridged, or missing.

## Thin-adapter rules

- Treat `AGENTS.md` as the canonical source for shared repository rules.
- Treat `.agents/skills/` as the canonical source for shared repository workflows.
- Keep `CLAUDE.md`, `.github/copilot-instructions.md`, and `.codex/AGENTS.md` as platform adapters, not competing rule stores.
- Keep generic harness capabilities in user-level config instead of mirroring them into repo-level `.github/**`.
- When a rule applies across tools, move it to `AGENTS.md` or a shared skill first, then keep only the platform-native delta here.

## Default workflow baseline

- For routine work, restore context from `AGENTS.md` plus `.agents/skills/bill-analyser-conventions/SKILL.md` before reaching for prompt or agent catalogs.
- Treat `.github/prompts/` as convenience entrypoints and `.github/agents/` as specialized escalations, not as competing repository-wide rule sources.

## Reviewer Subagent Contract

- When invoking `code-reviewer`, `python-reviewer`, or `security-reviewer`, always pass an explicit `Review Context`.
- `Review Context` must include `base_ref`, `head_ref`, `changed_files`, and `diff_text` (or representative patch hunks plus the refs needed to recover the rest).
- If you cannot provide either diff text or usable refs, report that review is blocked by missing scope instead of asking the reviewer to infer the whole repository.

## Session Completion

- Before ending a session, check whether the workspace still has staged or unstaged git diff.
- If any git diff remains, invoke `zh-conventional-commit-from-diff` and produce a Chinese Conventional Commit title for the current change set.
- Only skip this step when there is no staged or unstaged diff left.

## Interrupted-session recovery

- Copilot-side hooks refresh `.git/ai/last-session.md` after meaningful edits and again at stop time.
- If a chat is interrupted by network issues or there is no retry button, read `.git/ai/last-session.md`, inspect `git status` / `git diff`, then continue with the shared `session-resume` workflow in `.agents/skills/session-resume/SKILL.md`.

## Build and test reminder

For build and test commands, architecture boundaries, reviewer handoff rules, and session-completion expectations, follow [AGENTS.md](../AGENTS.md) first.
