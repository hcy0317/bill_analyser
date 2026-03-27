# CLAUDE.md

This file provides guidance to Claude Code when working with Bill Analyser.

## Start Here

1. Read [.github/copilot-instructions.md](.github/copilot-instructions.md).
2. Read [AGENTS.md](AGENTS.md) for the repository's agent-asset map and tool-specific entrypoints.
3. Use this file only for Claude-specific additions after the workspace rules above.

## Project Summary

- Backend: Python, Flask, aiosqlite
- Frontend: Vue 3, TypeScript, Vite
- Database: SQLite in WAL mode
- Core domains: bill import, categorization, budgeting, statistics, accounts, tags

## Guardrails

For project-wide architecture constraints, safety rules, and build/test commands, follow [.github/copilot-instructions.md](.github/copilot-instructions.md) instead of maintaining a second copy here.

## Claude Code Support Files

- Commands: [.claude/commands](.claude/commands)
- Rules: [.claude/rules](.claude/rules)
- Skills: [.claude/skills](.claude/skills)

When there is any ambiguity, follow [.github/copilot-instructions.md](.github/copilot-instructions.md).