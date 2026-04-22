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

The active Codex baseline now lives in the user-level profile at `~/.codex/config.toml`.
This repository should keep only Codex-specific discovery notes and repository deltas, not a second full harness copy.

## Codex-specific runtime layer

- Root `AGENTS.md` is the primary instructions source that Codex discovers automatically.
- User-level `~/.codex/config.toml`, `~/.codex/hooks.json`, `~/.codex/agents/*.toml`, and `~/.codex/harness/**` provide the active Codex runtime configuration.
- Repo-local `.codex/` should stay thin and only document Bill Analyser-specific Codex guidance that is not already covered by `AGENTS.md`.

## Multi-Agent Roles

- Prefer the user-level Codex custom agents for read-only exploration, review, planning, ultrawork orchestration, and other generic harness roles.
- If the repository ever needs a truly Bill Analyser-specific Codex agent again, add only that delta instead of mirroring the whole user-level harness back into `.codex/`.
