# Bill Analyser Codex Guide

This file supplements [AGENTS.md](AGENTS.md) with Codex-specific guidance.

## Start Order

1. Read [AGENTS.md](AGENTS.md) for the canonical shared repository rules.
2. Use this file only for Codex-specific runtime details.
3. Do not copy the full repository guide into `.codex/`; keep Codex adapters thin.

## Recommended Focus

- Prefer Rust runtime, TypeScript frontend, testing, API review, and documentation tasks.
- Use the repository skill in [.agents/skills/bill-analyser-conventions](.agents/skills/bill-analyser-conventions).
- Keep Codex work grounded in the current repository architecture rather than generic ECC defaults.
- If a session is interrupted, recover from `.git/ai/last-session.md` and the shared `.agents/skills/session-resume/SKILL.md` workflow before resuming edits.

## Session Completion

- Codex 侧没有仓库内 stop hooks 兜底时，仍然必须遵循 `AGENTS.md` 里的会话收尾规则。
- 只要结束会话时存在 git diff，就自动使用 `zh-conventional-commit-from-diff` 生成一条中文 Conventional Commit 标题，再结束本次会话。
- 具体 commit/PR 收尾策略以 `AGENTS.md` 为准：提交标题保持一行，小标题写入正文换行 bullet；切面 PR 尽量拆成多个可评审 commits；PR 标题必须使用 `type(scope): 主标题` 格式并描述功能域结果；PR body 使用 `### 目标` / `### 变更范围` / `### 验证证据` / `### 风险与开放门禁` 标准章节；合并时优先 squash，合并后删除来源分支；不要强制追加 OmX co-author trailer。

## MCP Baseline

The active Codex baseline usually lives in the user-level profile at `~/.codex/config.toml`.
This repository also keeps a thin fallback `.codex/config.toml` so CI / doctor checks still see the minimum multi-agent + MCP baseline when no home-level profile is available.
Keep the repo copy minimal; do not mirror the full user-level harness back into the repository.

## Codex-specific runtime layer

- Root `AGENTS.md` is the primary instructions source that Codex discovers automatically.
- User-level `~/.codex/config.toml`, `~/.codex/hooks.json`, `~/.codex/agents/*.toml`, and `~/.codex/harness/**` provide the active Codex runtime configuration.
- Repo-local `.codex/` should stay thin and only document Bill Analyser-specific Codex guidance that is not already covered by `AGENTS.md`.

## Multi-Agent Roles

- Prefer the user-level Codex custom agents for read-only exploration, review, planning, ultrawork orchestration, and other generic harness roles.
- If the repository ever needs a truly Bill Analyser-specific Codex agent again, add only that delta instead of mirroring the whole user-level harness back into `.codex/`.

## Cross-Platform Agent/Skill Sync

- 跨平台 skill 兼容约定：`~/.agents/skills/cross-platform-skill-sync/SKILL.md`（用户级 definitive）
- OpenCode ↔ Codex agent 映射表：`~/.config/opencode/harness/agent-manifest.json`
- 修改 Codex agents (`~/.codex/agents/*.toml`) 或 OpenCode skills (`.opencode/skills/*/SKILL.md`) 时，必须同步更新映射。
