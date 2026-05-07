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

如果任务主要在做页面布局、按钮、颜色、弹窗、表格或视觉一致性，再补看：

4. `.agents/skills/bill-analyser-ui-style-reference/SKILL.md`

它们分别负责：

- `AGENTS.md`：仓库级总规则
- `docs/PROJECT_OVERVIEW.md`：系统当前如何工作
- `bill-analyser-conventions`：仓库专属工作流与验证基线
- `bill-analyser-ui-style-reference`：仓库专属 UI 风格参考

如果不是复杂架构问题，不要一上来就在 prompts / agents 列表里乱翻。

## 一张表看懂入口分工

| 入口 | 默认用途 | 什么时候用 | 什么时候别用 |
|---|---|---|---|
| `AGENTS.md` | 仓库总规则 | 每次开始任务前 | 不要把它当变更日志 |
| `docs/PROJECT_OVERVIEW.md` | 架构/领域事实 | 改 API、导入、预算、统计、账户、分类时 | 不写会话流水账 |
| `bill-analyser-conventions` skill | 仓库专属 workflow | 全栈、导入、统计、金额、契约变更 | 不替代项目总规则 |
| `bill-analyser-ui-style-reference` skill | 仓库专属 UI 风格参考 | 改页面布局、按钮、颜色、表格、弹窗、响应式一致性时 | 不替代 Vuetify / Framework7 官方文档 |
| `add-parser-standard-flow` skill | 新增 parser workflow | 新增解析器、收紧 `ParserFactory` 检测、补 parser 对齐回归时 | 不要拿它代替通用导入调试或 API/DB 变更流程 |
| `/plan` | 复杂任务规划 | 跨模块功能、重构、需求不清 | 小改动别过度启动 |
| `/start-work` | 从已批准计划直接执行 | `/plan` 之后、已有 checklist 之后、恢复已确认方案时 | 没有批准计划时不要假装进入执行 |
| `/handoff` | 显式交接未完成工作 | 会话要暂停、还有 diff、需要给下个会话可恢复摘要时 | 不要拿它代替 `/verify` |
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
5. 如果已有批准计划或 checklist，用 `/start-work` 进入执行
6. 用 `/verify`
7. 再做评审

### 复杂改动

1. `/plan`
2. plan 被确认后用 `/start-work`
3. 必要时 `/tdd`
4. 分阶段实现与验证
5. `/verify`
6. `code-reviewer` / `security-reviewer`

## `/plan`、`/start-work`、`/handoff` 的关系

这三个入口现在构成一个更顺手的闭环：

- `/plan`：把需求和边界讲清楚
- `/start-work`：在**已有批准计划**前提下直接进入执行
- `/handoff`：在中断或暂停前显式交接当前状态

可以把它理解成：

`plan -> start-work -> verify -> handoff(如需暂停)`

如果你已经有明确 checklist，不一定非要重新 `/plan`；这时可以直接 `/start-work`。

如果你还没做完、但会话要停，就不要只留下一个模糊的聊天尾巴，优先 `/handoff`。

## `.git/ai/last-session.md` 和 `.git/ai/task-state.json`

现在恢复链有两份互补的状态文件：

- `.git/ai/last-session.md`：面向人读的最近会话快照
- `.git/ai/task-state.json`：面向工具和后续 prompt 的轻量任务状态

如果只是想快速看当前 task-state，可运行：

- `./.venv/Scripts/python.exe scripts/hooks/task_state.py --trigger manual --recent-file AGENTS.md --json`
- 或直接读取 `.git/ai/task-state.json`

前者回答“刚才发生了什么”，后者回答“当前任务现在卡在哪儿、下一步做什么”。

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

仓库 guard 只阻止极端破坏性操作，例如：

- 删除整盘或文件系统根目录
- 清空当前仓库、删除 `.git`、或执行强制工作区清理
- 删除 `data/bills.db*` / `data/config`，或对运行库执行显式清库 SQL
- 普通仓库外文件写入不再由 repo guard 拦截
- 如果当前机器存在 `~/.copilot/hooks/`，仓库还会通过 `scripts/hooks/copilot_global_hook_bridge.py` 额外桥接用户级 Copilot hooks（例如 config-protection / secret-scan / danger-guard）

### PostToolUse

在关键编辑后：

- 给出最相关的验证提示
- 刷新会话快照
- 刷新 `.git/ai/task-state.json`
- 如果存在用户级 `~/.copilot/hooks/`，还会桥接额外的全局质量提醒

### Stop

在会话收尾时：

- 如果工作区还有 diff，给出中文 Conventional Commit 标题建议
- 刷新会话快照
- 刷新 `.git/ai/task-state.json`
- 提示下一步恢复动作
- 如果存在用户级 `~/.copilot/hooks/`，还会桥接全局 stop hook（例如 session snapshot 持久化）

### `/hooks`

如果你想知道“到底有哪些 hooks 真的可用”，使用 `/hooks`：

- 它会区分仓库原生 hooks 与用户级 bridged hooks
- 它会说明哪些 hook 已接线、哪些只是磁盘上存在但未激活
- 它会给出下一步的健康检查或验证命令

## 会话中断后怎么继续

如果因为网络波动、窗口关闭或没有重试按钮而中断：

1. 先看 `AGENTS.md`
2. 再看 `docs/PROJECT_OVERVIEW.md`
3. 如果存在，读取 `.git/ai/last-session.md`
4. 如果存在，再读取 `.git/ai/task-state.json`
5. 运行 `git status`
6. 运行 `git diff`
7. 按 `.agents/skills/session-resume/SKILL.md` 的流程恢复任务

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
