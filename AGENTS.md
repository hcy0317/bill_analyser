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

- 后端主入口：`crates/bill-analyser-http/src/bin/bill_http_server.rs`
- 前端工程：`src/web`
- 当前运行态主链：`REST /api/...`

## Architecture

- Rust `bill_http_server` 是唯一 HTTP 运行时入口；不要重新增加 sidecar 或旧式透传兜底。
- 数据访问优先通过 Rust repository/runtime 层完成；不要把数据库逻辑散落到路由处理函数中。
- 新实现优先走 Rust crates + `REST /api/...`，不要重新引入 `/api/v1/*` 作为运行时主链。
- 时间、金额、账户、分类的前后端转换优先放在适配器/转换层，不要在路由或 store 中重复散落。

## Critical conventions

- 金额单位必须显式处理：后端核心存元，很多前端/API 交互用分；改动金额字段时必须人工复核一次元/分转换。
- 修改导入链路时，优先检查 parser、dedup、preview、learning 与 confirm 的 Rust 调用顺序是否仍然一致。
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

- 不要按全局进程名强杀运行时进程；需要停止服务时优先按端口或脚本精确定位。
- 不要提交本地数据库、日志、上传文件或密钥。
- CLI 提交 / 上传 / 会话快照必须尊重 `.gitignore`、`.git/info/exclude` 与 `core.excludesFile`；判断候选文件时优先使用 git-aware 枚举（如 `git ls-files --others --exclude-standard`），有疑问时先运行 `git check-ignore -v -- <path>`，若怀疑该路径已被追踪，再补 `git ls-files -- <path>` 核对。
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
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90
# 前端检查
Set-Location src\web
npm run lint
npm run test:coverage
```

提交前最低检查：受影响的 Rust/前端用例通过；只要交付的是代码改动，就必须补一条真实 coverage 命令并满足覆盖率 > 90%；如果改动了业务源码，被改业务代码自身也必须单独达到 90% 覆盖率（优先按 diff 改动的可执行行核算，缺少行级数据时按被改文件核算）；Rust 改动至少通过相关 `cargo test`，风险较高或工作区共享改动补 `cargo clippy --workspace --all-targets -- -D warnings`；前端改动至少通过 `npm run lint` 或最小构建验证；接口或金额字段变更时人工复核一次元/分转换。

## Default AI workflow

- 默认先读取 `AGENTS.md` 与 `docs/PROJECT_OVERVIEW.md`，再加载共享 skill `.agents/skills/bill-analyser-conventions/SKILL.md`。
- 如果任务主要在做页面布局、按钮样式、颜色、弹窗、表格或整体视觉一致性，再补读 `.agents/skills/bill-analyser-ui-style-reference/SKILL.md`。
- `prompts` 是快捷入口，`agents` 是专项能力；不要把它们当成第二套仓库级规则来源。
- 普通任务优先保持单 agent、小步修改、就地验证；只有在架构设计、显式代码评审、安全审查、构建故障、关键 E2E 等场景才升级为专项 agent。
- 按改动路径选择验证动作：
	- `crates/**`、`Cargo.toml`、`Cargo.lock`：先跑受影响 `cargo test`；共享 runtime/业务代码交付前必须运行 `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings` 与 `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90`
	- `src/web/**`：至少运行 `npm run lint`；若交付前端代码，还必须运行 `npm run test:coverage`，并满足总覆盖率 > 90% 以及被改业务代码自身覆盖率 > 90%
	- `.gitea/**`：先读 `.agents/skills/gitea-ci-cache-discipline/SKILL.md`，运行 YAML 解析、Rust-only source tree gate 与受影响 CI 本地等价命令
	- `.github/**`、`.agents/**`、`.claude/**`、`.codex/**`、`scripts/**`：运行 Rust-only source tree gate 与相关静态检查；对 hook/agent adapter 改动至少校验 JSON，并确认活跃配置没有引用已删除的 `scripts/hooks/**` 或 `scripts/agent_stack_health.py`；不要重新引入已删除的 sidecar/tooling 路径

## Audit gate for business-code changes

- 任何业务代码变更（包括 `crates/**` Rust 运行时代码，以及会影响业务行为、导入链路、预算/统计结果、API 契约的相关实现）在准备验收前，必须至少执行一次 Rust 完整覆盖率门禁：`cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90`
- 业务源码改动还必须单独核算被改代码覆盖率：优先用 `coverage.json` / `lcov.info` 按 diff 新增/修改的可执行行计算，改动行覆盖率必须 > 90%；缺少行级数据时，被改文件的文件级覆盖率必须 > 90%。
- 开发过程中可以先跑受影响用例做快速反馈，但这不能替代最终的全量测试验收。
- 只有在全量测试 / coverage gate 执行完成、全部通过、总覆盖率 > 90%，且被改业务代码覆盖率 > 90% 时，才可以视为通过审计验收。
- 如果没有执行全量测试，或全量测试 / coverage gate 存在任何失败，则该改动必须打回重做，不得以“局部测试通过”代替。

## Interrupted-session recovery

- hooks 会把最近一次工作快照写到 `.git/ai/last-session.md`。
- 如果会话因网络波动、窗口关闭或没有重试按钮而中断，先读取该快照，再查看 `git status` / `git diff`。
- 继续任务时优先使用共享 skill `.agents/skills/session-resume/SKILL.md`，基于快照、当前分支、HEAD 与工作区 diff 恢复上下文。
- 快照只记录分支、HEAD、路径、建议下一步和验证提示；不要写入密钥、环境变量或完整 diff。

## Asset inventory

- Copilot instructions, agents, prompts, and hooks: `.github/`
- Claude-specific runtime assets: `.claude/`
- Codex runtime assets: `.codex/`
- OpenCode runtime assets: `opencode.json`, `.opencode/`
- Shared skills: `.agents/skills/`
- Project UI style reference skill: `.agents/skills/bill-analyser-ui-style-reference/`
- Shared scripts: `scripts/`

### Cross-platform skill sync convention

- 任一平台（Claude Code / OpenCode / GitHub Copilot / Codex）修改 skills（用户级或项目级）后，必须确保全平台兼容。
- 约定详见：`~/.agents/skills/cross-platform-skill-sync/SKILL.md`（用户级 definitive 版本）
- OpenCode 与 Codex 的 agent 映射表：`~/.config/opencode/harness/agent-manifest.json`
- 共享 skill 优先放在 `.agents/skills/`（项目级）或 `~/.agents/skills/`（用户级），平台本地目录只做薄适配器。

## Session completion

- 每次会话结束前，只要当前工作区存在 staged 或 unstaged 的 git diff，就必须自动使用 `zh-conventional-commit-from-diff` 这个 skill。
- 生成结果必须给出一条基于当前 diff 的中文 Conventional Commit 标题；如果 staged diff 非空，优先基于 staged diff 生成。
- 多主题但仍属于同一提交意图的 diff，提交标题保持一行 `type(scope): 主标题`；关键子变更写到提交正文里的换行小标题 / bullet，不要堆在标题里。
- Plan-driven / OMX 切面进入 PR 前，若一个切面内存在可独立评审的规则、实现、测试或文档步骤，应优先拆成多个小 commits；不要为了省事把所有内容压成一个大提交。
- PR 标题必须使用 `type(scope): 主标题` 这种 Conventional Commit 大标题格式，并描述功能域切面的最终结果；不要直接复用第一个 commit 标题。PR body 必须使用标准章节 `### 目标`、`### 变更范围`、`### 验证证据`、`### 风险与开放门禁`，验证与门禁用 checklist；不要再使用裸 `Summary` / `Test plan` / `Open gates` 旧格式。
- PR 合并时优先使用 squash / 压缩提交；合并后应删除来源分支，平台支持自动删除时启用自动删除，否则在确认合并完成后删除远端 feature branch。
- hooks / gate / commit 模板不得强制或自动追加 `Co-authored-by: OmX <omx@oh-my-codex.dev>`；只有用户明确要求时才可添加该 trailer。
- 只有在确认没有任何 diff 时，才可以跳过这一步。

## Reference docs

- 详细架构与模块地图见 `docs/PROJECT_OVERVIEW.md`
- 人类可读的 AI 工作流说明见 `docs/AI_WORKFLOW.md`
- 页面布局、按钮、颜色与视觉一致性参考见 `.agents/skills/bill-analyser-ui-style-reference/SKILL.md`
- Copilot / VS Code 自定义入口见 `.github/`
- Claude Code 入口见 `CLAUDE.md` 与 `.claude/`
- Codex 入口见 `.codex/`
- OpenCode 入口见 `opencode.json`
