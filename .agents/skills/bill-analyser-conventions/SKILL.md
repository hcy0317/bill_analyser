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

## Default Execution Loop

1. Restore context from `AGENTS.md`, `docs/PROJECT_OVERVIEW.md`, and the current `git diff`.
2. Prefer one focused agent and minimal edits unless the task clearly needs planner/reviewer/security/build-fix escalation.
3. Add or update the most relevant regression test before risky behavior changes.
4. Implement the smallest change that satisfies the verified scenario.
5. Run path-sensitive verification before moving on.

## Verification Baseline

- `src/bill_analyser/**`：先跑受影响 pytest 与 pylint；代码交付前必须全量运行 `./.venv/Scripts/python.exe -m pytest --cov=src/bill_analyser --cov-report=term-missing tests/ -v`，并满足覆盖率 > 90%。
- `src/web/**`：至少在 `src/web` 下运行 `npm run lint`；如果交付了前端代码，还必须运行 `npm run test:coverage`，并满足覆盖率 > 90%。
- `.github/**`、`.agents/**`、`.claude/**`、`scripts/hooks/**`：运行 `./.venv/Scripts/python.exe scripts/agent_stack_health.py --mode repo` 与相关 hook / 健康检查 pytest。
- API 路由或契约相关改动要复核 `src/web/src/lib/services.ts` 与相关 store。
- 涉及金额字段、统计口径、导入链路时要人工复核元/分转换与调用顺序。
- Prefer minimal, focused changes over broad refactors.
- Before ending a session with any git diff, invoke `zh-conventional-commit-from-diff` and produce a Chinese Conventional Commit title for the current change set.

## Interrupted-session recovery

- If the session is interrupted, first inspect `.git/ai/last-session.md`.
- Compare the snapshot with current `git status` / `git diff`, then continue with `.agents/skills/session-resume/SKILL.md`.
- Treat the snapshot as metadata only; never store secrets, environment variables, or full patch contents inside it.

## Common Commands

- `./.venv/Scripts/python -m pytest tests/ -v`
- `./.venv/Scripts/python -m pylint src/core/*.py`
- `./start_backend.ps1`
- `./start_frontend.ps1`
