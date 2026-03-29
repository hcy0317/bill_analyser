---
name: 'python-hooks'
description: 'Python hooks extending common rules'
applyTo: '**/*.py'
---

# Python Hooks

> This file extends the common hooks rule with Python specific content.

## Repository Hook Baseline

- 仓库当前提供最小 guard hooks，入口在 `.github/hooks/*.json` 与项目级 `.claude/settings.json`
- 共享 guard 脚本位于 `scripts/hooks/pre_tool_repo_guard.py`

## Optional Local PostToolUse Hooks

If you want auto-formatting or deeper local-only automation, configure it in `~/.claude/settings.json` or `.claude/settings.local.json`:

- **black/ruff**: Auto-format `.py` files after edit
- **mypy/pyright**: Run type checking after editing `.py` files

## Warnings

- Prefer warning about `print()` statements in edited files (use `logging` module instead)
