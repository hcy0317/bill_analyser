# CLAUDE.md

This file is the Claude Code adapter for Bill Analyser.
Shared repository rules live in [AGENTS.md](AGENTS.md); keep this file Claude-specific and intentionally short.

## Start Here

1. Read [AGENTS.md](AGENTS.md) for the canonical shared repository rules.
2. Read `.claude/rules/` for path-scoped or Claude-only overlays.
3. Use this file only for Claude-specific additions; do not duplicate the full repository guide here.

## Project Summary

- Backend: Python, Flask, aiosqlite
- Frontend: Vue 3, TypeScript, Vite
- Database: SQLite in WAL mode
- Core domains: bill import, categorization, budgeting, statistics, accounts, tags

## Claude Code entrypoints

- Project instructions adapter: `CLAUDE.md`
- Path-scoped rules: `.claude/rules/`
- Subagents: `.claude/agents/`
- Claude-native skills and bridges: `.claude/skills/`
- Commands and workflows: `.claude/commands/`
- Permissions, hooks, plugins, and local overrides: `.claude/settings.json` and `.claude/settings.local.json`

## Claude-specific guidance

- If a rule is tool-agnostic, move it to `AGENTS.md` or a shared skill under `.agents/skills/` instead of growing this file.
- Prefer `.claude/rules/` for Claude path-scoped guidance; do not turn it into another full copy of `AGENTS.md`.
- If a reusable workflow should work in Claude Code, Copilot, OpenCode, and Codex, prefer `.agents/skills/` as the canonical source and keep `.claude/skills/` as a thin discovery bridge.
- Use `.claude/settings.json` for hard enforcement (permissions, hooks, plugin/MCP policy), not long-form behavioral guidance.
- For routine repository work, restore context from `AGENTS.md` plus `.agents/skills/bill-analyser-conventions/SKILL.md` before escalating to specialized agents.
- If a session is interrupted, inspect `.git/ai/last-session.md`, then resume with `.agents/skills/session-resume/SKILL.md`.

## Claude Code Support Files

- Commands: [.claude/commands](.claude/commands)
- Rules: [.claude/rules](.claude/rules)
- Skills: [.claude/skills](.claude/skills)

When there is any ambiguity, follow [AGENTS.md](AGENTS.md) first.