# Bill Analyser Codex Guide

This file supplements [AGENTS.md](AGENTS.md) with Codex-specific guidance.

## Start Order

1. Read [AGENTS.md](AGENTS.md) for the canonical shared repository rules.
2. Use this file only for Codex-specific runtime details.
3. Do not copy the full repository guide into `.codex/`; keep Codex adapters thin.

## Recommended Focus

- Prefer Python, TypeScript, testing, API review, and documentation tasks.
- Use the repository skill in [.agents/skills/bill-analyser-conventions](.agents/skills/bill-analyser-conventions).
- Keep Codex work grounded in the current repository architecture rather than generic ECC defaults.
- If a session is interrupted, recover from `.git/ai/last-session.md` and the shared `.agents/skills/session-resume/SKILL.md` workflow before resuming edits.

## Session Completion

- Codex 侧没有仓库内 stop hooks 兜底时，仍然必须遵循 `AGENTS.md` 里的会话收尾规则。
- 只要结束会话时存在 git diff，就自动使用 `zh-conventional-commit-from-diff` 生成一条中文 Conventional Commit 标题，再结束本次会话。

## MCP Baseline

The project-local Codex baseline is defined in [.codex/config.toml](.codex/config.toml).
It keeps a small MCP set aligned with the repository's VS Code setup.

## Codex-specific runtime layer

- Root `AGENTS.md` is the primary instructions source that Codex discovers automatically.
- `.codex/config.toml` contains Codex runtime configuration, MCP defaults, and Codex-specific agent wiring.
- `.codex/agents/*.toml` remains Codex-native because its schema is not shared with Copilot / Claude / OpenCode agent manifests.

## Multi-Agent Roles

- `.codex/agents/explorer.toml` for read-only evidence gathering
- `.codex/agents/reviewer.toml` for correctness and regression review
- `.codex/agents/docs-researcher.toml` for API and docs verification