# Bill Analyser AI 工作流说明

这份文档给人看，用来说明日常 agent / skill / verification 入口怎么协作。

## 默认从哪里开始

日常任务默认先看三样：

1. `AGENTS.md`
2. `docs/PROJECT_OVERVIEW.md`
3. `.agents/skills/bill-analyser-conventions/SKILL.md`

如果任务主要在做页面布局、按钮、颜色、弹窗、表格或视觉一致性，再补看：

4. `.agents/skills/bill-analyser-ui-style-reference/SKILL.md`

## 入口分工

| 入口 | 默认用途 | 什么时候用 |
|---|---|---|
| `AGENTS.md` | 仓库总规则 | 每次开始任务前 |
| `docs/PROJECT_OVERVIEW.md` | 架构/领域事实 | 改 API、导入、预算、统计、账户、分类时 |
| `bill-analyser-conventions` skill | 仓库专属 workflow | 全栈、导入、统计、金额、契约变更 |
| `bill-analyser-ui-style-reference` skill | 仓库专属 UI 风格参考 | 改页面布局、按钮、颜色、表格、弹窗、响应式一致性时 |
| `add-parser-standard-flow` skill | 新增 Rust parser workflow | 新增解析器、收紧 parser-first / parser parity 检测时 |
| `gitea-ci-cache-discipline` skill | Gitea CI 缓存治理 | 调整 `.gitea/workflows/ci.yml`、`actions/cache`、Rust/npm 缓存时 |
| `/plan` | 复杂任务规划 | 跨模块功能、重构、需求不清 |
| `/start-work` | 从已批准计划直接执行 | `/plan` 之后、已有 checklist 之后、恢复已确认方案时 |
| `/handoff` | 显式交接未完成工作 | 会话要暂停、还有 diff、需要给下个会话可恢复摘要时 |
| `/verify` | 默认验证入口 | 交付前、评审前、PR 前 |
| `code-reviewer` | 显式 diff 评审 | 有明确 diff 和 Review Context 时 |
| `security-reviewer` | 安全专项审查 | 鉴权、输入、密钥、API 边界 |
| `build-error-resolver` | 构建/类型/编译故障 | build/lint/type 已经红了 |

## 推荐流程

小改动：

1. 看 `AGENTS.md`
2. 看 `docs/PROJECT_OVERVIEW.md`（如果改动触及业务链路）
3. 小步修改
4. 按路径验证
5. 需要时做 diff review

中等改动：

1. 恢复上下文
2. 明确风险点
3. 先补最相关测试
4. 做最小实现
5. 用 `/verify`
6. 再做评审

复杂改动：

1. `/plan`
2. plan 被确认后用 `/start-work`
3. 必要时 `/tdd`
4. 分阶段实现与验证
5. `/verify`
6. `code-reviewer` / `security-reviewer`

## 验证基线

### `src/backend/**`

- 先跑受影响 `cargo test`
- Rust 集成/契约测试源码放在 `tests/backend/*`，不要再放回 crate-local `tests/` 目录
- 共享 runtime 或业务代码改动补：
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 最终业务验收跑：
  - `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90`

### `src/web/**`

- 至少在 `src/web` 下运行：
  - `npm run lint`
- 前端代码交付补：
  - `npm run test:coverage`
  - `npm run build`

### `.gitea/**`

- 先读 `.agents/skills/gitea-ci-cache-discipline/SKILL.md`
- 补 YAML 解析、Rust-only source tree gate 和受影响 CI 本地等价命令

### `.github/**` / `.agents/**` / `.claude/**` / `scripts/**`

- 保持入口薄适配，不复制整套仓库规则
- 运行 Rust-only source tree gate 与受影响静态检查

## Commit / PR 收尾规则

Plan-driven 或 OMX 切面不默认把本地历史 squash 成一个大提交。一个切面内如果存在可独立评审的规则、实现、测试、文档步骤，优先拆成多个小 commits，再建立同一个功能域 PR。

PR 标题必须使用 `type(scope): 主标题` 这种 Conventional Commit 大标题格式，并写功能域结果，不直接照搬第一个 commit 标题。PR body 统一使用 `### 目标`、`### 变更范围`、`### 验证证据`、`### 风险与开放门禁`，验证与门禁用 checklist；不要使用裸 `Summary` / `Test plan` / `Open gates` 非标准格式。PR 合并时优先 squash / 压缩提交；合并后删除来源分支，平台能自动删除就启用自动删除，否则确认 merge 后删除远端 feature branch。hooks / gate / commit 模板不得强制或自动追加 `Co-authored-by: OmX <omx@oh-my-codex.dev>`。

## 会话中断后怎么继续

如果因为网络波动、窗口关闭或没有重试按钮而中断：

1. 先看 `AGENTS.md`
2. 再看 `docs/PROJECT_OVERVIEW.md`
3. 如果存在，读取 `.git/ai/last-session.md`
4. 如果存在，再读取 `.git/ai/task-state.json`
5. 运行 `git status`
6. 运行 `git diff`
7. 按 `.agents/skills/session-resume/SKILL.md` 的流程恢复任务

快照只存元信息，不记录完整 patch 或密钥。

## 什么时候更新这份文档

当以下内容发生稳定变化时更新：

- 默认入口发生变化
- prompts / skills / agents / hooks 的角色边界发生变化
- 中断恢复流程发生变化
- 验证基线发生稳定变化

不要把这份文档写成会话日志、修复记录或 TODO 清单。
