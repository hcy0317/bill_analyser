---
name: 'typescript-hooks'
description: 'TypeScript hooks extending common rules'
applyTo: '**/*.ts,**/*.tsx,**/*.js,**/*.jsx,**/*.mjs,**/*.cjs,**/*.mts,**/*.cts,**/*.vue'
---

# TypeScript/JavaScript Hooks

> This file extends the common hooks rule with TypeScript/JavaScript specific content.

## Repository Hook Baseline

- 仓库当前提供最小 guard hooks，入口在 `.github/hooks/*.json` 与项目级 `.claude/settings.json`
- 共享 guard 脚本位于 `scripts/hooks/pre_tool_repo_guard.py`

## Optional Local PostToolUse Hooks

If you want auto-formatting or deeper local-only automation, configure it in `~/.claude/settings.json` or `.claude/settings.local.json`:

- **Prettier**: Auto-format JS/TS files after edit
- **TypeScript check**: Run `tsc` after editing `.ts`/`.tsx` files
- **console.log warning**: Warn about `console.log` in edited files

## Stop Hooks

- **console.log audit**: Optionally check modified files for `console.log` before session ends
