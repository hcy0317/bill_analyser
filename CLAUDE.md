# CLAUDE.md

This file provides guidance to Claude Code when working with Bill Analyser.

## Start Here

1. Read [AGENTS.md](AGENTS.md).
2. Read [.github/copilot-instructions.md](.github/copilot-instructions.md).
3. Treat the repository-specific guidance as higher priority than generic ECC conventions.

## Project Summary

- Backend: Python, Flask, aiosqlite
- Frontend: Vue 3, TypeScript, Vite
- Database: SQLite in WAL mode
- Core domains: bill import, categorization, budgeting, statistics, accounts, tags

## Guardrails

- Preserve the async bridge pattern in Flask routes.
- Keep database operations async and use aiosqlite.
- Treat REST as the primary runtime API chain.
- Handle money units explicitly: backend stores yuan, many frontend APIs use cents.
- Do not use destructive Python process-kill commands.

## Common Commands

- Start backend: `./start_backend.ps1`
- Start frontend: `./start_frontend.ps1`
- Stop services: `./停止服务器.ps1`
- Run tests: `./.venv/Scripts/python -m pytest tests/ -v`
- Lint core modules: `./.venv/Scripts/python -m pylint src/core/*.py`

## Claude Code Support Files

- Commands: [.claude/commands](.claude/commands)
- Rules: [.claude/rules](.claude/rules)
- Skills: [.claude/skills](.claude/skills)

When there is any ambiguity, follow [.github/copilot-instructions.md](.github/copilot-instructions.md).