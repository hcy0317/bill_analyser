---
name: bill-analyser-conventions
description: Repository-specific conventions for Bill Analyser. Use this for import, budgeting, statistics, and full-stack changes.
---

# Bill Analyser Conventions

## When to Use

Use this skill when you are:

- modifying Flask routes, services, or SQLite access
- changing Vue/TypeScript screens or stores
- working on bill import, categories, budgets, statistics, accounts, or tags
- validating API contracts, money-unit conversions, or async bridge behavior

## Core Rules

- Follow the repository entry files first: AGENTS.md and .github/copilot-instructions.md.
- Preserve the Flask-to-async bridge pattern.
- Keep database code async with aiosqlite.
- Treat REST as the primary runtime API chain.
- Keep yuan and cents conversions explicit.

## Verification Baseline

- Run targeted pytest coverage for affected areas.
- Use pylint for changed Python modules.
- Prefer minimal, focused changes over broad refactors.
- Before ending a session with any git diff, invoke `zh-conventional-commit-from-diff` and produce a Chinese Conventional Commit title for the current change set.

## Common Commands

- `./.venv/Scripts/python -m pytest tests/ -v`
- `./.venv/Scripts/python -m pylint src/core/*.py`
- `./start_backend.ps1`
- `./start_frontend.ps1`