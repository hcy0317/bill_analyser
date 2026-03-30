# Bill Analyser AI Customization Guide

`AGENTS.md` is the canonical cross-tool instruction layer for Bill Analyser.
If a rule is tool-agnostic and should apply across GitHub Copilot / VS Code, Claude Code, OpenCode, and Codex, put it here first.
Tool-specific entry files should stay thin and only add platform-specific discovery or runtime details.

## Canonical sources and thin adapters

### Canonical sources

- Shared repository rules: `AGENTS.md` (this file)
- Shared cross-tool skills: `.agents/skills/`

### Thin platform adapters

- Copilot / VS Code: `.github/copilot-instructions.md`
- Claude Code: `CLAUDE.md`
- Codex: `.codex/AGENTS.md`
- OpenCode: `opencode.json`

### Platform-native overlays

- Copilot instructions: `.github/instructions/`
- Copilot agents: `.github/agents/`
- Copilot prompts: `.github/prompts/`
- Copilot-local skills (migration / Copilot-only): `.github/skills/`
- Claude rules: `.claude/rules/`
- Claude agents: `.claude/agents/`
- Claude commands and settings: `.claude/commands/`, `.claude/settings*.json`
- Codex runtime config: `.codex/config.toml`, `.codex/agents/`
- Shared MCP catalog: `mcp-configs/ecc-mcp-servers.json`

### Layout rules

- Prefer `AGENTS.md` for universal repository guidance.
- Prefer `.agents/skills/` for new shared skills that should work across tools.
- Use `.github/instructions/` only when Copilot needs `applyTo`-style file scoping.
- Use `.claude/rules/` only for Claude path-scoped guidance or concise Claude-only reminders.
- Keep agent manifests platform-native; do not force one schema across `.github/agents`, `.claude/agents`, `.opencode/agents`, and `.codex/agents`.
- Avoid committed symlinks as the main strategy. Windows collaboration is the default expectation here; prefer thin wrappers and documented ownership.
- Keep `.github/copilot-instructions.md`, `CLAUDE.md`, and `.codex/AGENTS.md` smaller than this file.

## Project focus

Bill Analyser 是一个账单导入、去重、自动分类、预算与统计分析系统。

- 后端入口：`src/bill_analyser/api/app.py`
- 前端工程：`src/web`
- 当前运行态主链：`REST /api/...`

## Architecture

- Flask 路由层保持同步入口，通过独立事件循环桥接 async 服务；不要把路由层改成直接 async 运行模型。
- 数据库访问保持 `async def + aiosqlite`；不要把核心数据库逻辑改回同步。
- 新实现优先走 `REST /api/...`，不要重新引入 `/api/v1/*` 作为运行时主链。
- 时间、金额、账户、分类的前后端转换优先放在适配器/转换层，不要在路由或 store 中重复散落。

## Critical conventions

- 金额单位必须显式处理：后端核心存元，很多前端/API 交互用分；改动金额字段时必须人工复核一次元/分转换。
- 修改导入链路时，优先检查 `bill_service.py`、`smart_dedup.py`、`category_engine.py` 的调用顺序是否仍然一致。
- 修改 API 契约时，先确认 `src/web/src/lib/services.ts` 和相关 store 是否需要同步更新。
- 新增适配器优先使用中性模块命名，不要新增对 legacy `v1_*` 适配器文件的直接依赖。
- 保持变更聚焦，不顺手改无关历史问题。

## Reviewer Diff Contract

Across Copilot-, Claude-, and Codex-adjacent reviewer assets, treat review scope as explicit input instead of implicit context recovery.

- 调用 `code-reviewer`、`python-reviewer`、`security-reviewer` 时，必须显式传入 `Review Context`。
- `Review Context` 至少应包含：`base_ref`、`head_ref`、`changed_files`、`diff_text`（或代表性 patch hunk）。
- 如果 diff 过大，至少传入关键 hunk + refs；如果既没有 diff 也没有可用 refs，就应明确报告 review 被 scope 阻塞，而不是做泛化全仓库审查。

## PROJECT_OVERVIEW workflow

- 开始处理仓库问题前，先读取 `docs/PROJECT_OVERVIEW.md`，用它恢复当前功能域关系、主链路与模块边界。
- 只要代码改动改变了稳定的业务行为、接口契约或跨模块关系，就要同步更新 `docs/PROJECT_OVERVIEW.md` 对应章节。
- 写入 `docs/PROJECT_OVERVIEW.md` 时，只描述“系统现在如何工作”的业务/架构事实，不要写成本次会话日志、修复记录、时间线或 TODO 清单。

## Workspace boundaries

- 不要使用 `taskkill /f /im python.exe`。
- 不要提交本地数据库、日志、上传文件或密钥。
- 不要在没有充分理由的情况下改动构建产物、历史快照目录或第三方参考代码。
- 不要把同一套仓库级规则完整复制到多个平台入口文件里；共享规则优先回收到 `AGENTS.md`。
- Legacy `.cursor/` compatibility mirrors were intentionally removed；不要重新引入整套镜像目录。
- 如必须兼容 Cursor，仅允许保留单文件 `.cursor/mcp.json` 作为 MCP 薄适配器；不要继续扩展为 `.cursor/hooks*`、`.cursor/skills*`、`.cursor/commands*` 等镜像树。

## Build and test baseline

```powershell
# 启动
.\一键启动.ps1
.\start_backend.ps1
.\start_frontend.ps1

# 停止
.\停止服务器.ps1

# 测试
.\.venv\Scripts\python.exe -m pytest tests/ -v

# Python 静态检查
.\.venv\Scripts\python.exe -m pylint src/bill_analyser/core/*.py src/bill_analyser/api/routes/*.py

# 前端检查
Set-Location src\web
npm run lint
```

提交前最低检查：受影响的 pytest 用例通过；Python 改动至少通过对应模块的 pylint；前端改动至少通过 `npm run lint` 或最小构建验证；接口或金额字段变更时人工复核一次元/分转换。

## Default AI workflow

- 默认先读取 `AGENTS.md` 与 `docs/PROJECT_OVERVIEW.md`，再加载共享 skill `.agents/skills/bill-analyser-conventions/SKILL.md`。
- `prompts` 是快捷入口，`agents` 是专项能力；不要把它们当成第二套仓库级规则来源。
- 普通任务优先保持单 agent、小步修改、就地验证；只有在架构设计、显式代码评审、安全审查、构建故障、关键 E2E 等场景才升级为专项 agent。
- 按改动路径选择验证动作：
	- `src/bill_analyser/**`：先跑受影响 pytest / pylint；业务代码验收前必须全量运行 `./.venv/Scripts/python.exe -m pytest tests/ -v`
	- `src/web/**`：至少运行 `npm run lint`，必要时做最小构建验证
	- `.github/**`、`.agents/**`、`.claude/**`、`scripts/hooks/**`：运行 `./.venv/Scripts/python.exe scripts/agent_stack_health.py --mode repo` 与相关 hook / 健康检查 pytest

## Audit gate for business-code changes

- 任何业务代码变更（包括 `src/bill_analyser/**` 运行时代码，以及会影响业务行为、导入链路、预算/统计结果、API 契约的相关实现）在准备验收前，必须至少执行一次完整测试套件：`./.venv/Scripts/python.exe -m pytest tests/ -v`
- 开发过程中可以先跑受影响用例做快速反馈，但这不能替代最终的全量测试验收。
- 只有在全量 pytest 套件执行完成且全部通过时，才可以视为通过审计验收。
- 如果没有执行全量测试，或全量测试存在任何失败/错误，则该改动必须打回重做，不得以“局部测试通过”代替。

## Interrupted-session recovery

- hooks 会把最近一次工作快照写到 `.git/ai/last-session.md`。
- 如果会话因网络波动、窗口关闭或没有重试按钮而中断，先读取该快照，再查看 `git status` / `git diff`。
- 继续任务时优先使用共享 skill `.agents/skills/session-resume/SKILL.md`，基于快照、当前分支、HEAD 与工作区 diff 恢复上下文。
- 快照只记录分支、HEAD、路径、建议下一步和验证提示；不要写入密钥、环境变量或完整 diff。

## Asset inventory

- Copilot instructions, agents, prompts, and hooks: `.github/`
- Claude-specific runtime assets: `.claude/`
- Codex runtime assets: `.codex/`
- Shared skills: `.agents/skills/`
- Shared hook scripts: `scripts/hooks/`

## Session completion

- 每次会话结束前，只要当前工作区存在 staged 或 unstaged 的 git diff，就必须自动使用 `zh-conventional-commit-from-diff` 这个 skill。
- 生成结果必须给出一条基于当前 diff 的中文 Conventional Commit 标题；如果 staged diff 非空，优先基于 staged diff 生成。
- 只有在确认没有任何 diff 时，才可以跳过这一步。

## Reference docs

- 详细架构与模块地图见 `docs/PROJECT_OVERVIEW.md`
- Copilot / VS Code 自定义入口见 `.github/`
- Claude Code 入口见 `CLAUDE.md` 与 `.claude/`
- Codex 入口见 `.codex/`
- OpenCode 入口见 `opencode.json`