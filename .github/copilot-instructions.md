# Bill Analyser Copilot / VS Code Adapter

Shared repository rules live in [AGENTS.md](../AGENTS.md).
This file exists only for Copilot / VS Code-specific discovery and should stay thinner than `AGENTS.md`.

## What Copilot should load here

- Universal repository rules: `AGENTS.md`
- Always-on Copilot delta: this file
- File-scoped instructions: `.github/instructions/**/*.instructions.md`
- Custom agents: `.github/agents/*.agent.md`
- Prompt library: `.github/prompts/*.prompt.md`
- Copilot-native hooks (if used): `.github/hooks/*.json`
- Shared cross-tool skills: `.agents/skills/`

## Copilot-specific notes

- VS Code also discovers `AGENTS.md`, `CLAUDE.md`, `.claude/rules/`, `.claude/agents/`, `.agents/skills/`, and hook config from `.claude/settings.json`. Keep adapters thin to avoid repeating the same always-on context.
- Prefer `AGENTS.md` for repository-wide rules.
- Prefer `.instructions.md` files only when Copilot needs `applyTo`-style file scoping.
- New shared skills should prefer `.agents/skills/`; use `.github/skills/` only for Copilot-only skills or during migration.
- Use `.github/prompts/` for repeatable task templates rather than expanding `AGENTS.md` with prompt-like content.

## Current Bill Analyser overlays

- Modular workspace rules: `.github/instructions/ecc/`
- Copilot custom agents: `.github/agents/`
- Copilot prompts: `.github/prompts/`
- Existing Copilot-local skills: `.github/skills/`

## Reviewer Subagent Contract

- When invoking `code-reviewer`, `python-reviewer`, or `security-reviewer`, always pass an explicit `Review Context`.
- `Review Context` must include `base_ref`, `head_ref`, `changed_files`, and `diff_text` (or representative patch hunks plus the refs needed to recover the rest).
- If you cannot provide either diff text or usable refs, report that review is blocked by missing scope instead of asking the reviewer to infer the whole repository.

## Session Completion

- Before ending a session, check whether the workspace still has staged or unstaged git diff.
- If any git diff remains, invoke `zh-conventional-commit-from-diff` and produce a Chinese Conventional Commit title for the current change set.
- Only skip this step when there is no staged or unstaged diff left.

## Build and test reminder

For project build / test commands, architecture boundaries, reviewer handoff rules, and session-completion expectations, follow [AGENTS.md](../AGENTS.md).
