---
name: bill-analyser-conventions
description: Repository-specific conventions for Bill Analyser. Use this for import, budgeting, statistics, and full-stack changes.
---

# Bill Analyser Conventions

This is the canonical shared Bill Analyser workflow skill for cross-tool reuse.
If a tool needs its own discovery wrapper (for example `.claude/skills/...`), keep that wrapper thin and point back here.

## When to Use

Use this skill when you are:

- modifying Flask routes, services, or SQLite access
- changing Vue/TypeScript screens or stores
- working on bill import, categories, budgets, statistics, accounts, or tags
- validating API contracts, money-unit conversions, or async bridge behavior

## Core Rules

- Follow `AGENTS.md` first, then use tool-specific adapter files only for platform-native delta.
- Read `docs/PROJECT_OVERVIEW.md` before changing import, budgeting, statistics, accounts, categories, tags, or other cross-module flows so domain relationships are reloaded from the current repository baseline.
- Preserve the Flask-to-async bridge pattern.
- Keep database code async with aiosqlite.
- Treat REST as the primary runtime API chain.
- Keep yuan and cents conversions explicit.
- After stable business behavior, API contracts, or module relationships change, update the relevant section of `docs/PROJECT_OVERVIEW.md` immediately.
- Write `docs/PROJECT_OVERVIEW.md` as current-state business/architecture documentation, not as an update log, bug-fix diary, or dated session summary.

## Verification Baseline

- Run targeted pytest coverage for affected areas.
- After any business-code change, rerun the full repository pytest suite (`./.venv/Scripts/python.exe -m pytest tests/ -v`) before claiming audit acceptance.
- Targeted tests are for fast feedback only; audit acceptance requires the full pytest suite to finish green with no failures or errors.
- Use pylint for changed Python modules.
- Prefer minimal, focused changes over broad refactors.
- Before ending a session with any git diff, invoke `zh-conventional-commit-from-diff` and produce a Chinese Conventional Commit title for the current change set.

## Common Commands

- `./.venv/Scripts/python -m pytest tests/ -v`
- `./.venv/Scripts/python -m pylint src/core/*.py`
- `./start_backend.ps1`
- `./start_frontend.ps1`