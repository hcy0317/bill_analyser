# PROJECT_OVERVIEW 模块化拆分实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `docs/PROJECT_OVERVIEW.md`（306 行单文件）拆分为 12 个业务域子文件 + 1 个纯目录 index，使 agent 读取 overview 后可按摘要链接精准跳转，减少不必要的全量加载。

**Architecture:** 所有子文件平铺在 `docs/` 目录下，与 `PROJECT_OVERVIEW.md` 同级。内容只搬移不修改。`PROJECT_OVERVIEW.md` 改写为带摘要的链接目录，约 20 行。matching 内容从 §5.2 后半段独立提取；认证/安全相关条目从 §5.1/§5.3 跨节整合到单独文件。

**Tech Stack:** Markdown 文件操作，无代码变更，无测试需求，每任务一次 commit。

---

## 文件结构总览

| 操作 | 文件路径 | 来源 |
|---|---|---|
| 新建 | `docs/overview-positioning.md` | PROJECT_OVERVIEW.md §1（行 1–6） |
| 新建 | `docs/overview-architecture.md` | PROJECT_OVERVIEW.md §2（行 8–35） |
| 新建 | `docs/overview-backend.md` | PROJECT_OVERVIEW.md §3（行 38–93） |
| 新建 | `docs/overview-frontend.md` | PROJECT_OVERVIEW.md §4（行 96–112） |
| 新建 | `docs/overview-api-routes.md` | PROJECT_OVERVIEW.md §5.1（行 115–156） |
| 新建 | `docs/overview-import.md` | PROJECT_OVERVIEW.md §5.2（行 158–199，去掉 matching 行 180–189、193） |
| 新建 | `docs/overview-matching.md` | PROJECT_OVERVIEW.md §5.2 中 matching 段（行 180–189、193） |
| 新建 | `docs/overview-statistics.md` | PROJECT_OVERVIEW.md §5.3（行 200–223） |
| 新建 | `docs/overview-auth-security.md` | PROJECT_OVERVIEW.md §5.3 后半认证段（行 224–239） |
| 新建 | `docs/overview-database.md` | PROJECT_OVERVIEW.md §6（行 244–272） |
| 新建 | `docs/overview-ops.md` | PROJECT_OVERVIEW.md §7（行 275–287） |
| 新建 | `docs/overview-testing.md` | PROJECT_OVERVIEW.md §8（行 290–306） |
| 改写 | `docs/PROJECT_OVERVIEW.md` | 改为纯目录，约 20 行 |

> **边界说明：**
> - matching 提取边界：§5.2 中从"matching 域当前已额外提供 `GET /api/matching/sessions`"（行 180）开始到行 189，以及行 193（`GET /api/matching/sessions/<session_id>/candidates` 中 recurring 状态投影说明）。其余 §5.2 内容（三阶段导入、预览校验、学习域、LLM 推荐、行内编辑、transfer-decision、recurring-match、Annotation 筛选、投资识别链路）留在 `overview-import.md`。
> - auth/security 提取边界：§5.3 中从"认证入口主链已统一为..."（行 224）到"统计时间范围契约已收紧"（行 240）。这些条目虽物理上在 §5.3 文本中，但语义属于认证/2FA/token/backup/step-up/OAuth2，整合到 `overview-auth-security.md`。`overview-statistics.md` 只包含行 200–223（统计主链、汇率、用户数据、系统版本、账单写入、导入提交、交易列表等 legacy 收口条目）。

---

### Task 1: 创建 overview-positioning.md

**Files:**
- 新建: `docs/overview-positioning.md`

- [ ] **Step 1: 创建文件**

内容为 PROJECT_OVERVIEW.md §1（行 1–6）：

```markdown
# Bill Analyser 项目定位

Bill Analyser 是一个"多来源账单导入 + 智能去重 + 自动分类 + 多维统计分析"的全栈系统，核心目标是统一管理微信、支付宝与多家银行账单，并提供可编辑、可审计、可扩展的数据处理链路。
```

- [ ] **Step 2: Commit**

```bash
git add docs/overview-positioning.md
git commit -m "docs: extract positioning section to overview-positioning.md"
```

---

### Task 2: 创建 overview-architecture.md

**Files:**
- 新建: `docs/overview-architecture.md`

- [ ] **Step 1: 创建文件**

内容为 PROJECT_OVERVIEW.md §2（行 8–35），顶部加标题：

```markdown
# Bill Analyser 总体架构

## 2.1 分层结构
- **API 层（Flask）**：`src/bill_analyser/api/`
  - 同步路由处理 HTTP 请求
  - 通过事件循环桥接调用异步服务
- **业务层（Core）**：`src/bill_analyser/core/`
  - 账单导入编排、去重、分类、统计、汇率等核心逻辑
- **数据层（Database）**：`src/bill_analyser/core/db.py` + `src/bill_analyser/core/db_*.py`
  - `db.py` 只暴露公共 `Database` façade
  - 真实持久化能力按 runtime / schema / 业务域 mixin 拆分到多个 `db_*.py` 模块
  - 底层仍保持基于 `aiosqlite` 的异步数据库访问
- **前端层（Vue3 + TS）**：`src/web/src/`
  - 视图、状态管理（Pinia stores）、服务层（axios）

补充说明（2026-03-28）：
- 当前后端唯一源码根为 `src/bill_analyser/`。
- 仓库级 `src/api`、`src/core`、`src/parsers`、`src/utils`、`src/data`、`src/uploads` 等顶层阴影目录不再承载运行时代码或数据。

## 2.2 关键架构模式
- **异步桥接模式**：Flask 路由内创建独立事件循环调用 async 逻辑
- **REST 主链模式**：当前运行态主链统一收口到 REST（`/api/...`）
- **适配器/转换模式**：前后端字段、时间、金额单位统一转换

补充说明（2026-03-07）：
- 当前运行态已无 `/api/v1/*` 路由，也无 WSGI 级 URL rewrite 中间件。
- legacy 兼容已退出运行时代码树，当前仅残留在历史快照目录与针对遗留路径的回归测试约束中，不再体现在运行态路由表或适配器实现中。
```

- [ ] **Step 2: Commit**

```bash
git add docs/overview-architecture.md
git commit -m "docs: extract architecture section to overview-architecture.md"
```

---

### Task 3: 创建 overview-backend.md

**Files:**
- 新建: `docs/overview-backend.md`

- [ ] **Step 1: 创建文件**

内容为 PROJECT_OVERVIEW.md §3（行 38–93），顶部加标题 `# Bill Analyser 后端模块分布`，其余原样搬移。

- [ ] **Step 2: Commit**

```bash
git add docs/overview-backend.md
git commit -m "docs: extract backend modules section to overview-backend.md"
```

---

### Task 4: 创建 overview-frontend.md

**Files:**
- 新建: `docs/overview-frontend.md`

- [ ] **Step 1: 创建文件**

内容为 PROJECT_OVERVIEW.md §4（行 96–112），顶部加标题 `# Bill Analyser 前端模块分布`，其余原样搬移。

- [ ] **Step 2: Commit**

```bash
git add docs/overview-frontend.md
git commit -m "docs: extract frontend modules section to overview-frontend.md"
```

---

### Task 5: 创建 overview-api-routes.md

**Files:**
- 新建: `docs/overview-api-routes.md`

- [ ] **Step 1: 创建文件**

内容为 PROJECT_OVERVIEW.md §5.1（行 115–156，即"### 5.1 Flask 蓝图注册"到"前端 `services.ts` 与标签 store 已移除..."结束，不含 §5.2 及之后），顶部加标题 `# Bill Analyser API 路由与 REST 收口`，其余原样搬移。

- [ ] **Step 2: Commit**

```bash
git add docs/overview-api-routes.md
git commit -m "docs: extract API routes section to overview-api-routes.md"
```

---

### Task 6: 创建 overview-import.md

**Files:**
- 新建: `docs/overview-import.md`

- [ ] **Step 1: 创建文件**

内容为 §5.2 导入链路部分，**排除** matching 段落（行 180–189 和行 193）。具体包含：
- 行 158–179：v2 三阶段导入、legacy 兼容、导入预览校验交互、预览数据结构、分类规则刷新、dedup signal tooltip、导入学习域四层、学习 session 校验、CheckDataTab 入口、LLM analyze-transactions、LLM preview-recommend
- 行 190–199（行 190–192、194–199，跳过行 193）：preview 行内编辑、transfer-decision、recurring-match、Check Data 职责拆分、投资识别链路、Annotation 筛选说明

顶部加标题 `# Bill Analyser 导入链路`，其余原样搬移。

- [ ] **Step 2: Commit**

```bash
git add docs/overview-import.md
git commit -m "docs: extract import pipeline section to overview-import.md"
```

---

### Task 7: 创建 overview-matching.md

**Files:**
- 新建: `docs/overview-matching.md`

- [ ] **Step 1: 创建文件**

内容为从 §5.2 提取的 matching 段落（行 180–189 和行 193），顶部加标题 `# Bill Analyser Matching 域`，其余原样搬移：

- 行 180：`GET /api/matching/sessions/<session_id>/candidates` 导入会话级只读候选视图
- 行 181：`GET /api/matching/candidates` 统一只读入口
- 行 182：`POST /api/category-rules/migrate` 投资识别迁移与 investment-settings 退役
- 行 183：`billId` selector 混合 historical candidate 读模型
- 行 184：`GET /api/matching/bills/<bill_id>/feedback` bill-scoped 审查读口
- 行 185：`POST /api/matching/candidates/<candidate_id>/accept|reject|clear` generic 决策入口
- 行 186：`POST /api/matching/reconcile-history` 批量读取入口
- 行 187：历史正式账单后配对 MVP（manual-pair、DELETE pairs、GET pairs）
- 行 188：桌面端交易详情消费 historical matching 链路
- 行 189：`bill_learning_rule_suppressions` resolved-suppression 存储
- 行 193：`GET /api/matching/sessions/<session_id>/candidates` 中 recurring candidate 状态投影

- [ ] **Step 2: Commit**

```bash
git add docs/overview-matching.md
git commit -m "docs: extract matching domain section to overview-matching.md"
```

---

### Task 8: 创建 overview-statistics.md

**Files:**
- 新建: `docs/overview-statistics.md`

- [ ] **Step 1: 创建文件**

内容为 PROJECT_OVERVIEW.md §5.3 前半（行 200–223，即"### 5.3 统计与汇率"到"`src/bill_analyser/api/app.py` 中历史 `URLRewriteMiddleware` 已移除"），顶部加标题 `# Bill Analyser 统计与汇率`，其余原样搬移。

- [ ] **Step 2: Commit**

```bash
git add docs/overview-statistics.md
git commit -m "docs: extract statistics section to overview-statistics.md"
```

---

### Task 9: 创建 overview-auth-security.md

**Files:**
- 新建: `docs/overview-auth-security.md`

- [ ] **Step 1: 创建文件**

内容为 PROJECT_OVERVIEW.md §5.3 后半认证安全段（行 224–240），顶部加标题 `# Bill Analyser 认证与安全`，其余原样搬移：

- 行 224：认证入口主链（login/register/logout）
- 行 225：认证辅助入口（email verify/resend/forgot/reset）
- 行 226：用户资料 REST（profile/avatar）
- 行 227：验证邮件重发 REST
- 行 228：第三方登录 REST
- 行 229：云同步设置 REST
- 行 230：2FA 完整 REST 写接口
- 行 231：2FA 登录验证链路
- 行 232：2FA 恢复码持久化哈希存储
- 行 233：step-up 验证基础
- 行 234：备份主链（创建/下载/删除/恢复/预验证/加密）
- 行 235：认证域敏感安全动作审计
- 行 236：备份域审计
- 行 237：OAuth2 callback authorize
- 行 238：token 会话主链
- 行 239：认证错误契约
- 行 240：统计时间范围契约（涉及认证边界的400错误收紧）

- [ ] **Step 2: Commit**

```bash
git add docs/overview-auth-security.md
git commit -m "docs: extract auth and security section to overview-auth-security.md"
```

---

### Task 10: 创建 overview-database.md

**Files:**
- 新建: `docs/overview-database.md`

- [ ] **Step 1: 创建文件**

内容为 PROJECT_OVERVIEW.md §6（行 244–272），顶部加标题 `# Bill Analyser 数据库与数据流`，其余原样搬移。

- [ ] **Step 2: Commit**

```bash
git add docs/overview-database.md
git commit -m "docs: extract database section to overview-database.md"
```

---

### Task 11: 创建 overview-ops.md

**Files:**
- 新建: `docs/overview-ops.md`

- [ ] **Step 1: 创建文件**

内容为 PROJECT_OVERVIEW.md §7（行 275–287），顶部加标题 `# Bill Analyser 日志与运维`，其余原样搬移。

- [ ] **Step 2: Commit**

```bash
git add docs/overview-ops.md
git commit -m "docs: extract ops section to overview-ops.md"
```

---

### Task 12: 创建 overview-testing.md

**Files:**
- 新建: `docs/overview-testing.md`

- [ ] **Step 1: 创建文件**

内容为 PROJECT_OVERVIEW.md §8（行 290–306），顶部加标题 `# Bill Analyser 测试结构与质量门禁`，其余原样搬移。

- [ ] **Step 2: Commit**

```bash
git add docs/overview-testing.md
git commit -m "docs: extract testing section to overview-testing.md"
```

---

### Task 13: 改写 PROJECT_OVERVIEW.md 为纯目录

**Files:**
- 改写: `docs/PROJECT_OVERVIEW.md`

- [ ] **Step 1: 用以下内容完整替换 PROJECT_OVERVIEW.md**

```markdown
# Bill Analyser 项目总览

多来源账单导入 + 智能去重 + 自动分类 + 多维统计分析全栈系统。后端 Python/Flask/aiosqlite，前端 Vue 3/TypeScript/Vite，数据库 SQLite WAL 模式。

## 目录

- [项目定位](overview-positioning.md) — 系统目标、核心数据源（微信/支付宝/多家银行）
- [总体架构](overview-architecture.md) — 分层结构（API/Core/Database/Frontend）、异步桥接、REST 主链模式
- [后端模块](overview-backend.md) — API 路由、Core 业务逻辑、解析器工厂、Database façade 与 mixin 结构
- [前端模块](overview-frontend.md) — 视图（desktop/mobile）、Pinia stores、统一 axios 服务层
- [API 路由与 REST 收口](overview-api-routes.md) — Flask 蓝图注册、账户/标签/模板/预算/账单/分类各域 REST 收口进展
- [导入链路](overview-import.md) — v2 三阶段导入、预览校验交互、学习域（corpus/dual-head model/overlay）、LLM 推荐
- [Matching 域](overview-matching.md) — 转账/投资/learning 配对候选、generic accept/reject/clear、manual pair 管理、reconcile-history
- [统计与汇率](overview-statistics.md) — 统计主链、汇率 REST、用户数据管理、legacy 收口条目
- [认证与安全](overview-auth-security.md) — 认证/2FA/token/backup/step-up/OAuth2 全链路 REST 收口与审计
- [数据库与数据流](overview-database.md) — 主要业务表、导入三阶段临时表、导入处理顺序
- [日志与运维](overview-ops.md) — 统一日志入口、启停脚本
- [测试结构](overview-testing.md) — 测试分布、推荐验证命令
```

- [ ] **Step 2: Commit**

```bash
git add docs/PROJECT_OVERVIEW.md
git commit -m "docs: refactor PROJECT_OVERVIEW.md into modular index with domain sub-files"
```

---

## 自检

**Spec 覆盖检查：**
- ✅ §1 项目定位 → Task 1
- ✅ §2 总体架构 → Task 2
- ✅ §3 后端模块 → Task 3
- ✅ §4 前端模块 → Task 4
- ✅ §5.1 蓝图/REST 收口 → Task 5
- ✅ §5.2 导入链路（非 matching）→ Task 6
- ✅ §5.2 matching 段落 → Task 7
- ✅ §5.3 统计汇率（行 200–223）→ Task 8
- ✅ §5.3 认证安全（行 224–240）→ Task 9
- ✅ §6 数据库 → Task 10
- ✅ §7 运维 → Task 11
- ✅ §8 测试 → Task 12
- ✅ index 改写 → Task 13

**Placeholder 扫描：** 无 TBD/TODO/填写详情。Task 3/4/5/10/11/12 内容体积较大，步骤描述已明确"原样搬移"并给出行号范围，不需要额外代码示例。

**类型一致性：** 纯文档操作，无函数/类型引用。
