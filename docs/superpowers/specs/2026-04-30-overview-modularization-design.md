# Design: PROJECT_OVERVIEW.md 模块化拆分

**日期**：2026-04-30  
**状态**：已批准，待实施

---

## 背景

`docs/PROJECT_OVERVIEW.md` 当前约 306 行，单文件承载了项目定位、架构、后端/前端模块、API 对接关系（含导入链路、matching 域、统计汇率、认证安全）、数据库、运维、测试等全部内容。Agent 读取时必须加载整个文件，且无法按业务域精准定位。

---

## 目标

- 将 `PROJECT_OVERVIEW.md` 改为纯目录（index）
- 各业务域内容迁移到独立子文件
- 目录链接携带一句摘要，agent 读目录即可判断相关性，无需打开子文件

---

## 文件结构

所有文件放在 `docs/` 下，与现有 `PROJECT_OVERVIEW.md` 同级：

```
docs/
├── PROJECT_OVERVIEW.md          ← 改为纯目录（index）
├── overview-positioning.md      ← §1 项目定位
├── overview-architecture.md     ← §2 总体架构
├── overview-backend.md          ← §3 后端模块分布
├── overview-frontend.md         ← §4 前端模块分布
├── overview-api-routes.md       ← §5.1 蓝图注册 + REST 收口进展（不含认证/安全条目）
├── overview-import.md           ← §5.2 导入链路 + 预览校验 + 学习域
├── overview-matching.md         ← §5.2 中 matching 子域（独立提取）
├── overview-statistics.md       ← §5.3 统计与汇率
├── overview-auth-security.md    ← 认证/2FA/token/backup/step-up/OAuth2（整合自 §5.x）
├── overview-database.md         ← §6 数据库与数据流
├── overview-ops.md              ← §7 日志与运维
└── overview-testing.md          ← §8 测试结构与质量门禁
```

---

## 目录文件格式（PROJECT_OVERVIEW.md）

```markdown
# Bill Analyser 项目总览

多来源账单导入 + 智能去重 + 自动分类 + 多维统计分析全栈系统。

## 目录

- [项目定位](overview-positioning.md) — 系统目标、核心数据源
- [总体架构](overview-architecture.md) — 分层结构、异步桥接、REST 主链模式
- [后端模块](overview-backend.md) — API 路由、Core 业务、解析器、Database façade
- [前端模块](overview-frontend.md) — 视图、Pinia stores、服务层
- [API 路由与 REST 收口](overview-api-routes.md) — 蓝图注册、各域 REST 收口进展
- [导入链路](overview-import.md) — v2 三阶段导入、预览校验、学习域（corpus/model/overlay）
- [Matching 域](overview-matching.md) — 转账/投资/learning 配对、generic accept/reject、pair 管理
- [统计与汇率](overview-statistics.md) — 统计主链、汇率、用户数据管理
- [认证与安全](overview-auth-security.md) — 认证、2FA、token、backup、step-up、OAuth2
- [数据库与数据流](overview-database.md) — 主要业务表、导入处理顺序
- [日志与运维](overview-ops.md) — 日志系统、启停脚本
- [测试结构](overview-testing.md) — 测试分布、验证命令
```

---

## 内容迁移规则

### 1. 内容不删减
只搬移，不修改语义。原始文本原样迁移到对应子文件。

### 2. matching 从 §5.2 提取
§5.2 中从"matching 域当前已额外提供 `GET /api/matching/sessions/<session_id>/candidates`"开始的所有 matching 内容迁移到 `overview-matching.md`。§5.2 剩余部分（v2 三阶段导入、导入预览校验、学习域）留在 `overview-import.md`。

### 3. auth/security 整合
§5.1 后半（认证/2FA/token/backup/step-up/OAuth2 相关条目，即"认证入口主链"到"认证错误契约"部分）从 api-routes 内容中提取，集中放入 `overview-auth-security.md`。`overview-api-routes.md` 只保留蓝图注册与账户/标签/模板/预算/账单/分类等功能域的 REST 收口进展。

### 4. 子文件顶部标题
每个子文件顶部加一行 `# <标题>`，与目录链接文字保持一致。

---

## 边界决策

| 决策点 | 选择 | 理由 |
|---|---|---|
| 拆分粒度 | 按业务域（细） | 解决"太大"根本问题，每文件聚焦单一域 |
| 小节处理 | 全部独立成文件 | 结构最清晰，agent 定位无歧义 |
| 链接格式 | 带摘要的相对路径链接 | agent 读目录可判断相关性，无需打开子文件 |
| 存放位置 | `docs/` 同级 | 与现有文件一致，无需调整引用路径前缀 |
| auth 整合 | 跨节汇聚到单文件 | 避免 api-routes 文件依然庞大 |
