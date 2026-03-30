# Bill Analyser AI 工作流说明

这份文档给**人**看，不是给模型背八股。

目标只有三个：

1. 让团队知道默认该从哪里开始
2. 让 prompts / skills / agents / hooks 的边界足够清楚
3. 让会话中断后可以尽快恢复，不靠回忆硬扛

## 默认从哪里开始

日常任务默认先看三样：

1. `AGENTS.md`
2. `docs/PROJECT_OVERVIEW.md`
3. `.agents/skills/bill-analyser-conventions/SKILL.md`

它们分别负责：

- `AGENTS.md`：仓库级总规则
- `docs/PROJECT_OVERVIEW.md`：系统当前如何工作
- `bill-analyser-conventions`：仓库专属工作流与验证基线

如果不是复杂架构问题，不要一上来就在 prompts / agents 列表里乱翻。

## 一张表看懂入口分工

| 入口 | 默认用途 | 什么时候用 | 什么时候别用 |
|---|---|---|---|
| `AGENTS.md` | 仓库总规则 | 每次开始任务前 | 不要把它当变更日志 |
| `docs/PROJECT_OVERVIEW.md` | 架构/领域事实 | 改 API、导入、预算、统计、账户、分类时 | 不写会话流水账 |
| `bill-analyser-conventions` skill | 仓库专属 workflow | 全栈、导入、统计、金额、契约变更 | 不替代项目总规则 |
| `/plan` | 复杂任务规划 | 跨模块功能、重构、需求不清 | 小改动别过度启动 |
| `/tdd` | 测试先行实现 | 新行为、bug fix、关键逻辑 | 不是所有小文案都要强上 |
| `/verify` | **默认验证入口** | 交付前、评审前、PR 前 | 不适合只想快速看单文件问题 |
| `/quality-gate` | **快速路径检查** | 想快速检查单文件/单目录的格式、lint、type | 不能代替 `/verify` |
| `verification-loop` skill | 底层验证工作流 | 定制验证体系本身时 | 日常不要和 `/verify` 形成双入口心智负担 |
| `code-reviewer` | 显式 diff 评审 | 有明确 diff 和 Review Context 时 | 没 scope 时不要盲审 |
| `security-reviewer` | 安全专项审查 | 鉴权、输入、密钥、API 边界 | 不是每次改文档都要启用 |
| `build-error-resolver` | 构建/类型/编译故障 | build/lint/type 已经红了 | 不替代正常实现流程 |

## 推荐日常流程

### 小改动

1. 看 `AGENTS.md`
2. 看 `docs/PROJECT_OVERVIEW.md`（如果改动触及业务链路）
3. 小步修改
4. 按路径验证
5. 需要时做 diff review

### 中等改动

1. 先恢复上下文
2. 明确风险点
3. 先补最相关测试
4. 做最小实现
5. 用 `/verify`
6. 再做评审

### 复杂改动

1. `/plan`
2. 必要时 `/tdd`
3. 分阶段实现与验证
4. `/verify`
5. `code-reviewer` / `security-reviewer`

## `/verify`、`/quality-gate`、`verification-loop` 的关系

这是最容易重叠的一组入口，现在按下面理解就行：

### `/verify`

默认入口。

它做的是**仓库语义感知**的完整验证，例如：

- 后端改动 → pytest / pylint
- 前端改动 → `npm run lint`
- AI 定制层改动 → `agent_stack_health.py --mode repo`
- API 契约改动 → 检查 `src/web/src/lib/services.ts`
- 金额/统计改动 → 人工复核元/分

### `/quality-gate`

快速入口。

适合：

- “我刚改了一个文件，先看看 lint/type”
- “我只想对一个目录做快速质量扫描”

不适合：

- 最终验收
- PR 前完整检查
- 涉及 API 契约、金额语义、AI 定制层的综合验证

### `verification-loop`

底层 workflow。

你可以把它理解成 `/verify` 背后的方法论，而不是一个需要每天手动记住并与 `/verify` 竞争的独立入口。

## Hooks 现在会做什么

### PreToolUse

仓库 guard 会阻止明显危险或越界的操作，例如：

- `taskkill /f /im python.exe`
- 改动受保护的第三方/参考目录

### PostToolUse

在关键编辑后：

- 给出最相关的验证提示
- 刷新会话快照

### Stop

在会话收尾时：

- 如果工作区还有 diff，给出中文 Conventional Commit 标题建议
- 刷新会话快照
- 提示下一步恢复动作

## 会话中断后怎么继续

如果因为网络波动、窗口关闭或没有重试按钮而中断：

1. 先看 `AGENTS.md`
2. 再看 `docs/PROJECT_OVERVIEW.md`
3. 如果存在，读取 `.git/ai/last-session.md`
4. 运行 `git status`
5. 运行 `git diff`
6. 按 `.agents/skills/session-resume/SKILL.md` 的流程恢复任务

快照只存**元信息**，不会记录完整 patch 或密钥。

## 按路径选择验证动作

### `src/bill_analyser/**`

- 先跑受影响 pytest
- 跑 repository-baseline pylint
- 如果是业务运行时代码，最终必须全量跑：
  - `./.venv/Scripts/python.exe -m pytest tests/ -v`

### `src/web/**`

- 至少在 `src/web` 下运行：
  - `npm run lint`
- 复杂 UI/契约改动再补最小构建或测试

### `.github/**` / `.agents/**` / `.claude/**` / `scripts/hooks/**`

- 运行：
  - `./.venv/Scripts/python.exe scripts/agent_stack_health.py --mode repo`
- 再跑相关 hook / health pytest

## 什么时候更新这份文档

当以下内容发生稳定变化时更新：

- 默认入口发生变化
- prompts / skills / agents / hooks 的角色边界发生变化
- 中断恢复流程发生变化
- 验证基线发生稳定变化

不要把这份文档写成会话日志、修复记录或 TODO 清单。