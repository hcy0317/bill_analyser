# Bill Analyser 预想功能产品与技术规划（基于当前项目现状的增量路线）

> 文档目的：基于当前仓库实际能力、你提出的预想功能，以及同类产品的常见能力，给出一份**能落地、能拆任务、能排期、能映射到现有代码结构**的中长期规划。
>
> 规划原则：**不推翻现有 `Flask + sync route -> async service bridge + aiosqlite` 主架构**，优先采用增量演进，而不是重写式重构。

---

## 1. 规划结论先说清楚

当前项目并不是“功能几乎没有，需要重做一遍”，而是：

- **导入、去重、自动分类、三阶段预览、长期学习、2FA、备份** 都已经有了雏形；
- 真正缺的是：
  - **统一的配对域模型**；
  - **长期学习 -> 候选建议 -> 用户确认 -> 规则沉淀** 的闭环；
  - **安全/secret/备份恢复** 的工程化治理；
  - **新增解析器的标准流程**；
  - **更强的产品层能力**，例如周期交易、净资产、规则中心、异常洞察等。

因此，最优路线不是重做主链，而是基于现有：

- `src/bill_analyser/core/bill_service.py`
- `src/bill_analyser/core/smart_dedup.py`
- `src/bill_analyser/core/category_engine.py`
- `src/bill_analyser/parsers/factory.py`
- `src/bill_analyser/api/routes/auth.py`
- `src/bill_analyser/api/routes/backup.py`
- `src/bill_analyser/core/db.py`

继续长出新的“建议层、配对层、安全层、备份层、流程层”。

一句话概括本规划：

> **先做“安全可恢复 + 可解释 + 可确认”，再做“更智能”，最后做“更产品化”。**

---

## 2. 当前项目基线判断

## 2.1 已有能力与现成基础

### 导入与学习基础

当前项目已经具备以下重要基础：

- 导入主链已明确：`BillService -> SmartDeduplicationEngine -> CategoryEngine -> DB`
- 已有三阶段导入表：
  - `import_sessions`
  - `bills_parser_template`
  - `bills_preview`
- 已有长期学习相关表与方法：
  - `import_annotation_samples`
  - `import_learning_rules`
  - promotion / replay / annotation 相关逻辑
- 已有投资关键词设置与导入学习开关：
  - `users.import_learning_enabled`
  - `investment_platform_keywords`
  - `investment_product_keywords`
  - `investment_exclude_keywords`

这意味着“AI + 长期学习增强”不是从零设计，而是**已有规则学习底座，缺建议层与确认层**。

### 转账 / 投资识别基础

当前代码中已经存在：

- 转账配对
- 平台/银行去重
- 类似账单去重
- 分账单去重
- 跨批次转账配对
- 投资候选识别
- 现金存取转账检测

说明项目已经在账单关系识别方面有不少基础，只是还没有形成统一的“配对域模型 + 候选评分 + 用户反馈回灌”。

### 安全与认证基础

当前已存在：

- 登录 / 注册 / refresh / logout
- 2FA 状态、启用请求、启用确认、禁用、恢复码、登录验证
- token / session 管理
- 应用云设置与 profile 接口

问题不在“没有认证功能”，而在：

- 2FA 是否真正形成完整状态机；
- 是否对敏感操作形成 step-up 验证；
- `jwt_secret` / secret 管理还不够工程化。

### 备份基础

当前已存在：

- 本地备份创建
- 备份下载
- 删除备份
- 恢复备份
- 清理旧备份 **（已完成：2026-03-31，当前已支持 `.zip.enc`、按 `backup_records` 执行 retention，并回写删除状态）**

说明“备份”已具备产品入口，但还缺：

- 定时化
- 加密化
- 校验与恢复演练
- 更清晰的元信息与保留策略 **（已完成：2026-03-31，当前清理策略已优先基于 `backup_records` 与状态回写执行）**

### 解析器基础

当前已存在活动解析器：

- `wechat`
- `alipay`
- `icbc`
- `cmbc`
- `abc`
- `ccb`

并由 `ParserFactory` 统一调度。

所以“新增 parser 的 skill / 标准流程”非常适合现在做，属于**低风险高收益**事项。

---

## 2.2 真正的缺口是什么

下面是更准确的“缺口定义”：

### 缺口 1：长期学习没有完整的“建议层”

当前长期学习更偏：

- 收集人工标注；
- 沉淀到规则；
- 在导入时回放规则。

但还缺：

- 自动提炼候选关键词；
- LLM 对候选的归纳与清洗；
- 用户确认界面；
- 建议状态（pending / accepted / rejected）；
- 建议来源与解释信息。

### 缺口 2：配对能力散落，缺少统一生命周期模型

当前“转账/投资关系识别”更像若干算法点状能力，还缺：

- 候选池
- 候选评分
- 候选解释
- 后配对
- 拒绝反馈
- 配对结果回灌学习
- 跨批次配对的统一入口

### 缺口 3：安全是功能存在，但治理不足

包括：

- secret 保存方式不够稳健
- 2FA 可能只是功能块，不是安全闭环
- 缺少密钥轮换、恢复码哈希、敏感操作二次验证
- 缺少 secret provider 抽象

### 缺口 4：SQLite 适合现阶段，但数据库保护层不够完整

当前更应该做的是：

1. 应用层授权
2. secret 管理
3. 敏感字段加密
4. 备份加密
5. OS 级 ACL / 磁盘加密建议

而不是直接跳到“数据库必须立刻换成企业化方案”。

### 缺口 5：产品层缺少几个高价值能力

相对竞品，当前最值得补的功能是：

- 周期账单 / 订阅识别
- 日历与现金流视图
- 净资产 / 资产负债视图
- 规则中心与解释中心
- 异常交易/洞察

---

## 3. 总体设计原则

## 3.1 不推翻现有主架构

明确保留以下架构边界：

- Flask 路由仍然保持**同步入口**；
- 通过当前的 async bridge 调用异步服务；
- 数据库访问继续保持 `aiosqlite`；
- REST `/api/...` 继续作为运行态主链；
- 金额单位转换仍然在适配器/转换层明确处理，不散落在 UI 和 store 中。

## 3.2 确定性规则与概率性建议分层

未来能力必须分为两层：

### 决策层（确定性）

负责最终落库、最终分类、最终配对：

- `CategoryEngine`
- `SmartDeduplicationEngine`
- 未来新增 `matching/*` 的确定性服务

特点：

- 可解释
- 可回放
- 可测试
- 可拒绝

### 建议层（概率性 / AI）

只负责候选建议：

- 候选关键词
- 候选配对
- 候选归因
- 候选规则合并建议

特点：

- 必须可解释
- 必须可确认
- 默认不直接写正式规则
- 不直接取代现有规则引擎

## 3.3 所有学习都必须经过“用户确认”

无论是：

- 导入修正得到的学习；
- 配对成功得到的学习；
- LLM 生成的关键词候选；
- parser 标签与分类之间的新关联；

都应该先进入：

- `pending`
- 用户确认
- 再 promotion 到正式规则

这样可以避免系统越来越“聪明”，但用户越来越不信任它。

## 3.4 parser 元信息必须标准化

未来 parser 除了吐出标准账单字段之外，建议统一输出受控标签，例如：

- `parser:wechat`
- `parser:alipay`
- `channel:credit_card`
- `channel:wallet`
- `txn_family:transfer`
- `txn_family:investment`
- `counterparty_type:broker`
- `record_origin:bank_statement`

这些标签未来将服务于：

- 分类增强
- 转账配对
- 投资事件识别
- 后配对推荐
- 规则解释

**（已完成后端契约 MVP：2026-04-08，当前 `StandardBill`、`bills_parser_template`、`bills_preview` 已贯通 `parser_tags`，并且 `/api/bills/parse_import` 会返回 `parserSource + parserTags`，`BillService.get_import_preview()` / `/api/bills/import/v2/dedup` 会返回 `preview_parser_id + preview_parser_tags`。）**

## 3.5 安全与备份采用分层增强

建议分三层推进：

### 第一层：短期必做

- `.env` / 环境变量外置 secret
- 2FA 真闭环
- 恢复码哈希化
- 备份校验
- 审计日志

### 第二层：中期增强

- Secret provider 抽象
- 敏感字段加密
- 备份加密
- 定时备份
- 恢复演练

### 第三层：长期可选

- Vault 接入
- SQLCipher / 全量数据库透明加密评估
- WebAuthn

---

## 4. 推荐总体技术栈

## 4.1 后端推荐技术路线

### 保持现有主栈

- Web：Flask
- Async 服务：现有 async service 层
- DB：SQLite + `aiosqlite`
- 测试：pytest

### 建议新增的轻量能力

- 文本相似度：`rapidfuzz`（优先于重型 ML）
- 加密：`cryptography`
- Secret 本地管理（Windows 优先）：`keyring` 或 DPAPI 封装
- 定时任务：
  - 本地单机优先：Windows Task Scheduler + CLI
  - 应用内补充可选：`APScheduler`
- LLM 适配：OpenAI-compatible provider 抽象

### 不建议现在就大幅引入的重型依赖

- 不建议当前就引入 Celery / Redis 做后台任务主链
- 不建议立刻切换 PostgreSQL 只为做权限控制
- 不建议立即做全量向量数据库 / embedding 主导方案

## 4.2 前端推荐技术路线

当前前端仍然建议沿用：

- Vue 3
- TypeScript
- 现有服务层 `src/web/src/lib/services.ts`
- 现有 import / user settings / budget / statistics 页面体系

建议新增的前端模块主要集中在：

- 导入预览里的“候选建议”与“后配对”交互
- 安全中心 / 2FA / 恢复码 / 备份设置
- 规则中心 / 学习建议中心
- 周期账单 / 日历 / 净资产视图

## 4.3 安全技术栈建议

### 短期

- 环境变量管理 secret
- JWT key 通过环境变量注入
- `pyotp` 继续作为 TOTP 基础
- 恢复码哈希化（不要仅保留明文缓存）

### 中期

- `cryptography` 负责敏感字段/备份加密
- `keyring` / DPAPI 作为 Windows 本地安全增强
- Secret provider 抽象：
  - env
  - local key store
  - vault adapter（可选）

### 长期

- Vault
- WebAuthn
- SQLCipher 评估

## 4.4 LLM 技术栈建议

### 定位

LLM 不作为最终分类器，而作为：

- 关键词候选生成器
- 相似关键词归并器
- 规则解释生成器
- 人工复核摘要器

### 适配策略

建议做统一 provider 层：

- `base_provider.py`
- `openai_compatible_provider.py`

不要直接在业务服务里散落某个供应商 SDK 调用。

### 风险控制

- 默认关闭，用户或管理员配置后启用
- 发送前做脱敏 / 裁剪
- 记录 suggestion source = `llm`
- 所有 LLM 结果都进入 `pending`，不得直接生效

---

## 5. 建议新增的模块结构

下面是建议的**增量结构**，尽量贴合当前仓库：

```text
src/bill_analyser/
├─ api/
│  └─ routes/
│     ├─ auth.py
│     ├─ backup.py
│     ├─ bills.py
│     ├─ categories.py
│     ├─ learning.py              # 已落地：独立学习建议中心路由（跨会话挖掘/接受/拒绝 + 规则管理）
│     ├─ matching.py              # 当前已提供 unified candidates 读取、session 候选读取、历史账单 mixed candidates 读取、manual-pair、pair delete、persisted pair list 与 multi-family reconcile-history 批量编排；generic accept/reject 当前已覆盖 preview transfer/recurring/investment/learning 与历史 transfer/investment/learning 的最小切片
│     ├─ recurring.py             # 已落地：GET /api/recurring/suggestions + POST detect + POST accept + POST reject
│     ├─ calendar.py              # 已落地：GET /api/calendar/events?start_date&end_date — 按日聚合交易 + recurring 预测投影
│     ├─ networth.py              # 已落地：GET /api/networth/snapshot — 按账户分组聚合资产/负债/净值
│     ├─ rules.py                 # 已落地：GET /api/rules/overview — 统一聚合 learning rules + category keywords + recurring rules
│     ├─ insights.py              # 已落地：GET /api/insights/anomalies?months — 大额交易 + 重复扣款 + 月度分类偏离检测
│     └─ security.py              # 可选：安全中心聚合接口
├─ core/
│  ├─ bill_service.py
│  ├─ smart_dedup.py
│  ├─ category_engine.py
│  ├─ db.py
│  ├─ recurring_detection.py      # 已落地：周期模式检测算法（分组→频率→置信度评分）
│  ├─ db_recurring_suggestions.py # 已落地：DatabaseRecurringSuggestionsMixin — recurring suggestions CRUD
│  ├─ import_learning/            # 建议新增
│  │  ├─ sample_collector.py
│  │  ├─ keyword_miner.py
│  │  ├─ suggestion_service.py
│  │  ├─ promotion_service.py
│  │  └─ rule_explainer.py
│  ├─ matching/                   # 当前已提供 preview.matching、session candidate 投影与历史 transfer+learning candidate helper
│  │  ├─ feature_extractor.py
│  │  ├─ candidate_generator.py
│  │  ├─ score_model.py
│  │  ├─ transfer_matcher.py
│  │  ├─ investment_matcher.py
│  │  ├─ reconciliation_job.py
│  │  └─ feedback_promoter.py
│  ├─ security/                   # 建议新增
│  │  ├─ secret_provider.py
│  │  ├─ jwt_keyring.py
│  │  ├─ totp_service.py
│  │  ├─ recovery_code_service.py
│  │  ├─ authorization.py
│  │  └─ audit_service.py
│  ├─ backup/                     # 建议新增
│  │  ├─ backup_service.py
│  │  ├─ restore_service.py
│  │  ├─ retention.py
│  │  └─ verify_restore.py
│  └─ llm/                        # 建议新增
│     ├─ base_provider.py
│     ├─ openai_compatible_provider.py
│     ├─ prompt_templates.py
│     └─ privacy_filter.py
├─ parsers/
│  ├─ factory.py
│  ├─ wechat.py
│  ├─ alipay.py
│  ├─ icbc.py
│  ├─ cmbc.py
│  ├─ abc.py
│  ├─ ccb.py
│  └─ base.py
└─ utils/
   ├─ config.py
   ├─ constants.py
   └─ logger.py
```

前端建议新增或扩展：

```text
src/web/src/
├─ lib/
│  └─ services.ts               # 已新增 getMatchingPairs() / reconcileMatchingHistory() / createManualPair() 三个 matching 域服务方法；已新增 getLearingSuggestions / generateLearningSuggestions / acceptLearningSuggestion / rejectLearningSuggestion / batchAcceptLearningSuggestions / getLearningRules / toggleLearningRule / deleteLearningRule 八个学习中心服务方法；已新增 getRecurringSuggestions / detectRecurringPatterns / acceptRecurringSuggestion / rejectRecurringSuggestion 四个周期发现方法；已新增 getCalendarEvents / getNetWorthSnapshot / getRulesOverview / getAnomalies 四个 Phase4 服务方法
├─ stores/
│  ├─ importLearning.ts          # 建议新增
│  ├─ learning.ts                # 已落地：composition API store，管理 suggestions / rules / loading / error / tab / filter，提供 loadSuggestions / generateSuggestions / acceptSuggestion / rejectSuggestion / batchAcceptSuggestions / loadRules / toggleRule / deleteRule
│  ├─ matching.ts                # 已落地：composition API store，管理 pairs / loading / error / pairTypeFilter，提供 loadPairs / deletePair / createManualPair
│  ├─ recurring.ts               # 已落地：composition API store，管理 suggestions / loading / error / filter / pagination，提供 loadSuggestions / detect / accept / reject
│  ├─ securityCenter.ts          # 建议新增
│  └─ backup.ts                  # 可选新增
├─ views/desktop/
│  ├─ transactions/import/
│  │  ├─ ImportDialog.vue
│  │  ├─ tabs/ImportTransactionCheckDataTab.vue
│  │  ├─ dialogs/PairCandidateDialog.vue          # 建议新增
│  │  └─ dialogs/LearningSuggestionDialog.vue     # 建议新增
│  ├─ user/settings/
│  │  └─ tabs/
│  │     ├─ UserTwoFactorAuthSettingTab.vue
│  │     ├─ UserSecuritySettingTab.vue
│  │     ├─ UserDataManagementSettingTab.vue
│  │     └─ UserBackupSettingTab.vue              # 建议新增
│  ├─ learningcenter/ListPage.vue                 # 已落地：学习中心两标签页（建议/规则），支持生成建议、批量接受、单条接受/拒绝、规则启用/禁用/删除，路由 /learning/center
│  ├─ matching/BillPairingCenterPage.vue          # 已落地为 pairingcenter/ListPage.vue：配对中心列表页，支持类型侧栏过滤、删除确认、空状态，路由 /pairing/list
│  ├─ recurring/RecurringRulesPage.vue            # 建议新增
│  ├─ recurring/DiscoverPage.vue                  # 已落地：周期模式发现页面，支持自动检测 + 接受/拒绝，路由 /recurring/discover
│  ├─ calendar/CalendarPage.vue                   # 已落地：月历网格视图，按日聚合收支 + 月度汇总 + recurring 预测标记，路由 /calendar
│  ├─ networth/NetWorthPage.vue                   # 已落地：净资产视图，资产/负债列表 + 净值卡片，路由 /networth
│  ├─ rules/RuleCenterPage.vue                    # 已落地：规则中心三标签页（learning/keywords/recurring），路由 /rules/center
│  ├─ insights/InsightsPage.vue                   # 已落地：异常洞察，卡片列表 + 时间范围选择 + 严重程度分类，路由 /insights
│  └─ assets/NetWorthPage.vue                     # 建议新增（已由 networth/NetWorthPage.vue 替代）
└─ models/
   ├─ learning_center.ts         # 已落地：LearningSuggestion / LearningRule / BatchAcceptResponse / GenerateSuggestionsResponse 接口与 normalizeSuggestion() / normalizeRule() / normalizeBatchAcceptResponse() / normalizeGenerateResponse() 转换函数
   ├─ import_learning.ts         # 建议新增
   ├─ bill_matching.ts           # 已落地：已新增 BillMatchingPairBillSummary / BillMatchingPairDetail / MatchingPairsResponse / ReconcileHistorySummary / ReconcileHistoryBillResult / ReconcileHistoryResponse 接口与 normalizeMatchingPairsResponse() 转换函数
   ├─ recurring_suggestion.ts    # 已落地：RecurringSuggestion / RecurringSuggestionsResponse / RecurringDetectResponse / RecurringAcceptResponse 接口与 normalizers
   └─ security.ts                # 建议新增
```

---

## 6. 分阶段实施计划（Roadmap）

## 6.1 阶段 0：安全和基础元数据先打牢

### 目标

先补齐安全最短板，为后续智能增强提供稳定落点。

### 本阶段重点

1. 完整梳理并补齐 2FA 状态机
2. `jwt_secret` / token secret 外置 **（已完成：2026-03-30，当前加载顺序为环境变量 > `.env` > `server_config.json`）**
3. 恢复码哈希化，不仅仅明文缓存 **（已完成：2026-03-30，已落地数据库哈希持久化、一次性消费、重生成旧码失效）**
4. 定义 parser tags 标准 schema
5. 定义配对 reason / learning suggestion reason 的统一 JSON 结构
6. 备份功能补充校验信息、保留策略和恢复验证入口
7. 审计关键动作 **（已完成：2026-03-31，当前已覆盖 2FA 与备份创建/删除/恢复/恢复预验证关键动作审计）**

### 预计改动点

- `src/bill_analyser/api/routes/auth.py`
- `src/bill_analyser/api/routes/backup.py`
- `src/bill_analyser/core/db.py`
- 新增：
  - `src/bill_analyser/core/security/*`
  - `src/bill_analyser/core/backup/*`

### 建议新增数据结构

#### 建议扩展 users / sessions / auth logs 使用方式

重点不是再堆很多字段，而是让以下流程完整：

- 登录 -> need2FA -> verify -> issue session
- disable 2FA -> 必须验证密码/二次因子
- regenerate recovery code -> 原恢复码全部失效

#### 建议新增或扩展审计结构

可基于已有 `audit_logs`，增加关注事件：

- `2fa_enabled`
- `2fa_disabled`
- `2fa_recovery_regenerated` **（已完成：2026-03-30）**
- `secret_rotated`
- `backup_created`
- `backup_restored`
- `backup_restore_verified`

### 推荐技术栈

- `cryptography`
- `keyring`（Windows 本地增强，可选）
- 环境变量

### 风险

- 登录链路改动容易带来兼容问题
- 恢复码逻辑如果不仔细，容易“看起来安全，实际上失效”

### 验收标准

- 开启 2FA 后，登录必须经过二次验证才能拿到正式 token
- 恢复码为一次性、不可回读明文
- secret 已可通过 `.env` 或部署环境注入
- 备份可生成校验信息并支持恢复预验证 **（已完成：2026-03-31）**

### 优先级

- **最高优先级**
- **难度：中等**
- **收益：非常高**

---

## 6.2 阶段 1：长期学习增强（先确定性，再 LLM）

### 目标

把“长期学习”从当前的规则回放能力，升级为：

- 样本收集
- 候选关键词挖掘
- 用户确认
- 规则沉淀
- 导入回放

的完整闭环。

### 设计策略

先做两层：

#### 第一层：确定性候选挖掘

通过以下来源生成关键词候选：

- `import_annotation_samples`
- 用户手工分类修正
- 已确认配对结果
- 高置信度 parser tags

生成：

- 候选关键词
- 来源样本
- 命中次数
- 建议分类
- 置信度
- 解释信息

#### 第二层：LLM 做归纳器

LLM 只负责：

- 清洗冗余候选
- 归并近义词
- 推荐更适合写入规则的关键词短语
- 生成建议说明

LLM 不直接做最终入库。

### 建议新增模块

- 当前最小落地结构：
  - `src/bill_analyser/core/db_import_learning.py`：session-scoped dry-run learning suggestions 聚合（基于 `import_annotation_samples + bills_preview + import_learning_rules`），并复用 `promote_import_annotation_samples_to_learning(..., preview_ids=...)` 作为按来源 preview 精确提升的写侧底座；同时包含独立学习建议中心的 5 个跨会话 DB mixin 方法：`mine_learning_suggestions()`、`get_learning_suggestions()`、`count_learning_suggestions()`、`accept_learning_suggestion()`、`reject_learning_suggestion()`
  - `src/bill_analyser/core/bill_service.py`：learning suggestions DTO / summary 包装，同时支持 `get_import_learning_suggestions(session_id, preview_updates?, preview_ids?)` 这条 dry-run 薄桥；当传入 `preview_updates[]` 时会先保存当前选择的 annotation samples，再把 suggestions 按本次 preview 来源做子集过滤；`promote_session_annotations_to_learning(...)` 继续保持 `preview_updates[]` 优先、`previewIds[]` 精确提升、显式空选择为零提升的语义
  - `src/bill_analyser/api/routes/bills.py`：`/api/bills/import/v2/learning/<session_id>/suggestions` 当前同时承载 GET 只读 suggestions 与 POST 选中预览 suggestions 预览；`/api/bills/import/v2/learning/<session_id>/promote` 继续作为正式提升写接口，既支持 `preview_updates[]`，也支持可选 `previewIds[]`
  - `src/bill_analyser/api/routes/learning.py`：独立学习建议中心路由，注册在 `/api/learning`，提供跨会话建议挖掘、列表、接受/拒绝，以及学习规则管理（列表、启用/禁用、删除）

- `src/bill_analyser/core/import_learning/sample_collector.py`
- `src/bill_analyser/core/import_learning/keyword_miner.py`
- `src/bill_analyser/core/import_learning/suggestion_service.py`
- `src/bill_analyser/core/import_learning/promotion_service.py`
- `src/bill_analyser/core/llm/*`

### 建议新增表

#### `import_learning_suggestions`

当前已落地持久化字段（基于 composite_hash 的跨会话建议挖掘）：

- `id`
- `user_id`
- `match_type` (固定 `'composite'`)
- `normalized_match_value` (composite hash)
- `learned_type`
- `learned_category_id`
- `learned_source_account_id`
- `learned_destination_account_id`
- `sample_count`
- `source_session_ids_json`
- `status` (`pending` / `accepted` / `rejected`)
- `created_at`
- `updated_at`
- UNIQUE 约束：`(user_id, match_type, normalized_match_value)`

#### `import_learning_feedback_events`

建议字段：

- `id`
- `suggestion_id`
- `user_id`
- `action` (`accepted` / `rejected` / `edited_before_accept`)
- `payload_json`
- `created_at`

### API 建议

当前最小已落地接口：

- `GET /api/bills/import/v2/learning/<session_id>/suggestions`
- `POST /api/bills/import/v2/learning/<session_id>/suggestions`（当前请求体支持 `preview_updates[]` 与可选 `previewIds[]`；它会先把当前导入预览选择保存为 session annotation samples，再按所选 preview 来源过滤 dry-run suggestions，供导入预览弹窗确认）
- `POST /api/bills/import/v2/learning/<session_id>/promote`（当前请求体支持 `preview_updates[]`，也支持可选 `previewIds[]` 以精确提升当前 session 下已存在 annotation samples 的指定来源；显式空 `previewIds[]` 返回零提升；缺失 session 或跨用户访问统一返回 `404 Import session not found`）

独立学习建议中心已落地接口（`/api/learning` 蓝图）：

- `GET /api/learning/suggestions` — 列表查询，支持 `status` 过滤与 `page`/`page_size` 分页
- `POST /api/learning/suggestions/generate` — 触发跨会话标注挖掘，基于 composite_hash 聚合，返回 `{total_annotations, mined, created, skipped_conflict, skipped_existing}`
- `POST /api/learning/suggestions/<id>/accept` — 接受建议并提升为 `import_learning_rules` 长期规则，返回 `{status, rule_id}`
- `POST /api/learning/suggestions/<id>/reject` — 拒绝建议
- `POST /api/learning/suggestions/batch-accept` — 批量接受建议（最多 100 条），返回 `{accepted[{id, ruleId}], failed[{id, error}], acceptedCount, failedCount}`
- `GET /api/learning/rules` — 学习规则列表，支持 `enabled_only` 与分页
- `PUT /api/learning/rules/<id>/toggle` — 切换规则启用/禁用
- `DELETE /api/learning/rules/<id>` — 删除规则

### 前端

当前最小前端入口已先落在导入预览：`ImportTransactionCheckDataTab.vue` 通过 `ImportLearningSuggestionDialog.vue` 调用 `services.ts -> /api/bills/import/v2/learning/<session_id>/suggestions|promote`，先展示当前所选 preview 的 grouped suggestions，再按用户勾选的 suggestion 汇总 `sourcePreviewIds` 做精确提升；两条会话级接口都先校验 `import_sessions` 的当前用户可见性，缺失 session 或跨用户访问统一返回 `404 Import session not found`。

独立学习中心前端已落地（路由 `/learning/center`，导航入口位于配对中心之后）：

- 页面：`src/web/src/views/desktop/learningcenter/ListPage.vue`，双标签页布局（建议 / 规则）
- 建议标签页：状态过滤（pending/accepted/rejected）、全选、批量接受、单条接受/拒绝、匹配模式与建议动作展示
- 规则标签页：启用/禁用切换、删除、应用次数统计
- 类型模型：`src/web/src/models/learning_center.ts`（LearningSuggestion / LearningRule / BatchAcceptResponse / GenerateSuggestionsResponse + normalizer）
- Store：`src/web/src/stores/learning.ts`（composition API，管理 suggestions / rules / loading / error / tab / statusFilter）
- i18n：en.json / zh_Hans.json 已补齐 Learning Center 相关词条

### 推荐技术栈

- `rapidfuzz`
- 规则 + 词频 + 归一化
- OpenAI-compatible provider（可选）

### 风险

- LLM 结果可能不稳定
- 如果没有“pending -> accept”状态，错误规则会污染主分类体系

### 验收标准

- 可以生成候选关键词而不直接污染正式规则
- 用户可确认后写入 `import_learning_rules`
- 导入时仅消费已确认规则
- 可追溯 suggestion 的来源和拒绝原因

### 优先级

- **高优先级**
- **难度：中等**
- **收益：高**

---

## 6.3 阶段 2：统一转账 / 投资配对域，支持后配对与跨批次配对

### 目标

把当前分散的转账/投资关系识别，收口为统一的“配对域”：

- 候选生成
- 候选评分
- 候选解释
- 用户确认
- 用户拒绝
- 后配对
- 跨批次历史比对
- 回灌学习

### 设计策略

### 配对域应独立于去重域，但串联在导入主链中

推荐链路：

1. parser 输出标准账单 + parser tags
2. `SmartDeduplicationEngine` 继续负责重复消除
3. 新增 `matching/candidate_generator` 生成候选配对
4. 新增 `transfer_matcher` / `investment_matcher` 做评分和解释
5. 预览页展示候选结果
6. 用户确认后写入正式关系
7. 配对结果回灌长期学习

### 为什么转账和投资要放一起规划

因为两者共享很多底层能力：

- 金额关系
- 时间窗口
- 方向
- 账户关系
- parser tags
- counterparty / description hints

但两者不同点在于：

- 转账往往偏 1:1
- 投资更容易出现“事件组”（买入 + 手续费 + 划转）

所以实现时建议：

- 第一步先把数据模型设计成可扩展；
- 第一版仍优先落 1:1 和 1:n 候选，不要一开始做太复杂的图搜索。

### 建议新增模块

- `src/bill_analyser/core/matching/feature_extractor.py`
- `src/bill_analyser/core/matching/candidate_generator.py`
- `src/bill_analyser/core/matching/score_model.py`
- `src/bill_analyser/core/matching/transfer_matcher.py`
- `src/bill_analyser/core/matching/investment_matcher.py`
- `src/bill_analyser/core/matching/reconciliation_job.py`
- `src/bill_analyser/core/matching/feedback_promoter.py`

### 建议新增表

#### `bill_pair_candidates`

建议字段：

- `id`
- `user_id`
- `session_id`
- `pair_type` (`transfer` / `investment`)
- `source_bill_ref`
- `target_bill_ref`
- `confidence_score`
- `status` (`pending` / `accepted` / `rejected` / `expired`)
- `reason_json`
- `feature_json`
- `created_at`
- `updated_at`

其中 `bill_ref` 可以先设计成统一结构，既能指向：

- `bills_preview.id`
- `bills.id`
- `bills_parser_template.id`

也就是说，不一定都绑定正式账单。

#### `bill_pair_links`

建议字段：

- `id`
- `user_id`
- `pair_type`
- `left_bill_id`
- `right_bill_id`
- `source` (`manual`)
- `created_at`
- `updated_at`

- 当前这张表承载正式账单的最小 pair 关系：已覆盖 `transfer/manual` 与 `investment/manual` 两类 pair link；如后续扩展 auto / import_review / post_review 或 reason_json，再分阶段追加。

#### `bill_pair_feedback`

当前已落地字段：

- `id`
- `candidate_id`
- `user_id`
- `action` (`accept` / `reject` / `manual_override`)
- `payload_json`
- `created_at`

当前这张表是 append-only 的写侧事件流：已记录 historical formal-bill `transfer / investment` family 的 generic `accept|reject` 结果写回；目前不承载 preview family，也尚未扩到 formal-bill learning family。

### API（当前已落地 + 后续待补）

当前已落地：

- `GET /api/matching/candidates`
- `POST /api/matching/candidates/<id>/accept`
- `POST /api/matching/candidates/<id>/reject`
- `POST /api/matching/candidates/<id>/clear`
- `GET /api/matching/sessions/<session_id>/candidates`
- `GET /api/matching/bills/<bill_id>/candidates`
- `GET /api/matching/bills/<bill_id>/feedback`
- `POST /api/matching/manual-pair`（请求体为 `{ billId, candidateBillId, pairType? }`，`pairType` 默认 `transfer`，当前支持 `transfer | investment`）
- `DELETE /api/matching/pairs/<id>`
- `GET /api/matching/pairs`
- `POST /api/matching/reconcile-history`

其中 `GET /api/matching/candidates` 当前是 selector-based 的统一只读入口：请求必须二选一携带 `sessionId` 或 `billId`。`sessionId` 分支复用导入会话级候选读模型，返回与 `/api/matching/sessions/<session_id>/candidates` 等价的 `data{session_id, summary, candidates[]}`；`billId` 分支复用历史正式账单混合候选读模型，返回与 `/api/matching/bills/<bill_id>/candidates` 等价的 `data{billId, linkedPair, candidates[]}`，其中 `candidates[]` 当前可能同时包含 `transfer`、`investment` 与 `learning` family。它当前只统一 read-side，不承载 manual-pair 或 reconcile-history 写语义。

其中 `GET /api/matching/bills/<bill_id>/feedback` 当前是 historical formal-bill 的 bill-scoped feedback 读口：它会在当前用户下返回 `data{billId, events[]}`，其中每条 event 至少包含 `id / candidateId / action / createdAt / payload`，并按最新事件优先排序。该接口当前读取 `bill_pair_feedback` 中与目标 bill 相关的 append-only formal-bill feedback 事件，覆盖 `transfer / investment` family 的 generic accept|reject 结果；它是 pairing center / audit view 的历史动作视图，不改变 `GET /api/matching/pairs` 的读模型，也不扩到 preview family 或 formal-bill learning family。

 其中 `POST /api/matching/candidates/<id>/accept` 当前已支持 `preview:<preview_id>:transfer`、`preview:<preview_id>:recurring`、`preview:<preview_id>:investment`、`preview:<preview_id>:learning`、`bill:<billId>:transfer:<candidateBillId>`、`bill:<billId>:investment:<candidateBillId>` 与 `bill:<billId>:learning:<ruleId>:<ruleRevision>`。`preview:<preview_id>:transfer` 分支继续复用预览转账决策 `accept` 语义，并要求请求体携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)`；成功后返回 `data{candidateId, action, previewId, sessionId, preview[]}`。`preview:<preview_id>:recurring` 分支复用 preview recurring bind 写路径，要求请求体携带 `recurringId` 与 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)`；成功后返回 `data{candidateId, action, previewId, sessionId, recurringId, preview[]}`，并把当前 preview 重新绑定到目标 recurring candidate，使 session 级 recurring candidate 状态投影回 `confirmed`。之所以也要求 `reviewStatus`，是因为 recurring bind/clear 仍可能清理 transfer review feedback，所以 stale guard 需要把 transfer review 状态一起纳入快照。`preview:<preview_id>:investment` 分支复用 preview-scoped investment feedback 写路径，要求请求体携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)`；成功后返回 `data{candidateId, action, previewId, sessionId, preview[]}`，并把 `bills_preview.preview_matching_feedback_json.investment` 写为 `{review_status:"accepted", suppressed:false}`。该语义不会覆盖当前 `preview_type / category / recurring` 字段；follow-up 的 session 级 investment candidate 仍会保留，但状态会投影为 `accepted`。`preview:<preview_id>:learning` 分支复用 preview-scoped learning feedback 写路径，要求请求体携带 `ruleId` 与 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId, sourceAccountId, destinationAccountId)`；服务端只会接受当前 preview learning 读模型仍然暴露的同一条 rule。成功后返回 `data{candidateId, action, previewId, sessionId, preview[]}`，并把该 rule 已配置的 learned `type / category / source_account / destination_account` 结果应用到 `bills_preview.preview_type / preview_main_category / preview_sub_category / preview_source_account_id / preview_destination_account_id`，同时把 `bills_preview.preview_matching_feedback_json.learning` 写为 `{review_status:"accepted", suppressed:false, rule_id, previous_preview, applied_preview}`。follow-up 的 session 级 learning candidate 会继续保留，并把状态投影为 `accepted`。`bill:<billId>:transfer:<candidateBillId>` 分支继续复用历史正式账单 `manual-pair` 写路径，在当前用户下建立 1:1 transfer/manual pair，并返回 `data{candidateId, action, pair}`；成功后还会向 `bill_pair_feedback` 追加一条 `action="accept"` 事件，记录 `candidate_id` 与最小 pair payload。`bill:<billId>:investment:<candidateBillId>` 分支会写入 `bill_pair_links(pair_type='investment', source='manual')`，在当前用户下建立 1:1 investment/manual pair，并返回 `data{candidateId, action, pair}`；成功后同样会向 `bill_pair_feedback` 追加一条 `action="accept"` 事件。follow-up 的 `GET /api/matching/bills/<bill_id>/candidates` 与 `GET /api/matching/candidates?billId=...` 会返回 `linkedPair.pairType="investment"` 且 `candidates=[]`。`bill:<billId>:learning:<ruleId>:<ruleRevision>` 分支则会把该 candidate 视为一次有效 resolve：若 rule 对当前正式账单存在字段差异，就更新 `bills.type / main_category / sub_category / source_account_id / destination_account_id`；若 rule 已与当前 bill 对齐，则 accept 仍会成功返回当前 bill，不再报 no-op 错误。两种情况下它都会记录 `import_learning_rule_logs(action=accepted)`、增加该 rule 的 usage 计数，并把该 bill/rule 写入 resolved suppression，避免后续 bill selector 继续重复暴露同一 learning candidate。该接口当前不承载 reconcile-history 语义。

其中 `POST /api/matching/candidates/<id>/reject` 当前已扩到三个 preview family：

- `preview:<preview_id>:transfer`：继续复用预览转账决策 `reject` 语义，并要求请求体携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)`；成功后返回 `data{candidateId, action, previewId, sessionId, preview[]}`。reject 后该 transfer candidate 仍会保留在 session 级候选读模型里，但状态会投影为 `rejected`。
- `preview:<preview_id>:investment`：当前已支持 preview family 级 investment reject；它要求请求体携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)`，成功后返回 `data{candidateId, action, previewId, sessionId, preview[]}`。该写路径只会把 preview-scoped feedback 写入 `bills_preview.preview_matching_feedback_json.investment{review_status:"rejected", suppressed:true}`，不会覆盖 `preview_type / category / recurring` 当前值；follow-up 的 session 级 investment candidate 仍会保留，但状态会投影为 `rejected`。
- `preview:<preview_id>:learning`：当前已支持 preview family 级 learning reject；它要求请求体携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId, sourceAccountId, destinationAccountId)`，成功后返回 `data{candidateId, action, previewId, sessionId, preview[]}`。当该 learning candidate 仍处于 pending/rejected 轨道时，该写路径只会把 preview-scoped feedback 写入 `bills_preview.preview_matching_feedback_json.learning{review_status:"rejected", suppressed:true}`，不会覆盖 `preview_type / category / recurring` 当前值；如果它此前已经通过 generic accept 应用过 learning rule，则 reject 只会在当前 preview 仍匹配那次 accept 的 `applied_preview` 时恢复更早的 `previous_preview` snapshot；若用户在 accept 之后手工改过 learning 管辖字段，则 reject 会保留这些较新的手工编辑，只把状态投影为 `rejected`。follow-up 的 session 级 learning candidate 仍会保留，但状态会投影为 `rejected`。
- `preview:<preview_id>:recurring`：当前是 preview family 级 recurring reject 第一刀，复用 preview recurring clear 写路径，并要求请求体携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)`；成功后返回 `data{candidateId, action, previewId, sessionId, preview[]}`。该语义会清除当前 preview 上的 recurring 绑定，而不是记录 per-recurringId 的 suppressed 状态；follow-up 的 recurring candidate 仍会保留在 session 级候选读模型里，但状态会从 `confirmed` 回到 `pending`。
- `bill:<billId>:transfer:<candidateBillId>`：当前已支持 historical formal-bill transfer family 的 generic reject；成功后返回最小 `data{candidateId, action}`，并把这对 logical pair 持久化写入 `bill_transfer_pair_suppressions`，同时向 `bill_pair_feedback` 追加一条 `action="reject"` 事件。后续 `GET /api/matching/bills/<bill_id>/candidates` 与 `GET /api/matching/candidates?billId=...` 会直接过滤掉该 pair，且该 suppressed logical pair 之后也不能再通过 generic accept / manual pair 重新建立 pair。
- `bill:<billId>:investment:<candidateBillId>`：当前已支持 historical formal-bill investment family 的 generic reject；成功后返回最小 `data{candidateId, action}`，并把该 logical pair 的 investment suppress 持久化写入 `bill_investment_pair_suppressions`，同时向 `bill_pair_feedback` 追加一条 `action="reject"` 事件。后续 `GET /api/matching/bills/<bill_id>/candidates` 与 `GET /api/matching/candidates?billId=...` 会继续保留同一对账单的 transfer family，但过滤掉对应 investment family。
- `bill:<billId>:learning:<ruleId>:<ruleRevision>`：当前已支持 historical formal-bill learning family 的 generic reject；成功后返回最小 `data{candidateId, action}`，并把该 bill/rule suppress 持久化写入 `bill_learning_rule_suppressions`。后续 `GET /api/matching/bills/<bill_id>/candidates` 与 `GET /api/matching/candidates?billId=...` 会过滤掉该 rule 级 learning candidate；当同一 candidate 之后通过 generic accept 被应用到正式账单时，对应 suppress 也会被清理。

其中 `POST /api/matching/candidates/<id>/clear` 当前已支持 `preview:<preview_id>:learning` 与 `preview:<preview_id>:investment` 的 generic clear。`preview:<preview_id>:learning` 分支要求请求体携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId, sourceAccountId, destinationAccountId)`，成功后返回 `data{candidateId, action, previewId, sessionId, preview[]}`。当 preview 仍与上一次 learning accept 的 `applied_preview` 一致时，clear 会恢复更早的 `previous_preview` snapshot；若用户在 accept 之后手工改过 learning 管辖字段，则 clear 会保留这些较新的手工编辑，只清除 `preview_matching_feedback_json.learning`，并让 session 级 learning candidate 状态回到 `pending`。`preview:<preview_id>:investment` 分支则要求请求体携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)`，成功后同样返回 `data{candidateId, action, previewId, sessionId, preview[]}`，并只清除 `bills_preview.preview_matching_feedback_json.investment`。当 live investment signal 仍存在时，follow-up 的 preview 行与 session 级 investment candidate 都会把状态重新投影回 `pending`；该写路径依旧不会改写 `preview_type / main_category / sub_category / recurring` 等 preview 业务字段。

其他未支持 family 继续统一返回 `400 Candidate family not supported`。

历史正式账单 transfer candidates 现已具备稳定 `candidateId`，格式为 `bill:<billId>:transfer:<candidateBillId>`；该标识跟随当前 anchor bill 视角生成，用于给后续 generic accept/reject 与 pairing center 提供稳定候选身份，而不改变现有 score / 排序 / linkedPair 语义。

其中 `GET /api/matching/pairs` 当前会返回当前用户下已持久化的 `formal bill + manual pair` 列表，响应结构为 `data{pairs[]}`；每条 pair 包含 `id / pairType / source / leftBillId / rightBillId / createdAt / updatedAt`，以及 `leftBill / rightBill` 的正式账单最小摘要。它当前至少会暴露 `transfer/manual` 与 `investment/manual` 两类 pair，统一从 `bill_pair_links` 的 manual 子集投影而来；`bill_pair_feedback` 当前仍不参与该 pair list 读模型，而是通过 `GET /api/matching/bills/<bill_id>/feedback` 提供 bill-scoped 的 append-only 审查视图。

其中 `POST /api/matching/reconcile-history` 当前是历史正式账单候选读模型的批量编排入口：请求体必须携带非空 `billIds[]`，重复 id 会按首次出现顺序去重；可选携带 `families[]` 参数（允许值 `transfer / investment / learning`，默认 `["transfer"]`），用于指定本次批量读取包含的 candidate family。接口会逐条复用 `GET /api/matching/bills/<bill_id>/candidates` 的正式账单读模型，并返回 `data{summary{billCount, candidateCount, linkedPairCount}, results[]}`；其中每个 `results[]` 项都沿用单账单候选读取的结构 `billId / linkedPair / candidates[]`。运行时结构 map 为 `POST /api/matching/reconcile-history -> api/routes/matching.py::_parse_reconcile_history_request() -> BillService.reconcile_matching_history(bill_ids, families) -> 逐条 BillService.get_formal_bill_matching_candidates()`。若任一 `billId` 不存在或不属于当前用户，则整个请求返回 `404 Bill not found`；非法请求体或非法 `billIds` 返回 `400`。它当前是 all-or-nothing 的 batch read orchestration，而不是新的 pairing / reject / reconcile write 语义。

历史正式账单 bill selector 当前已不再是 transfer-only 读模型：`GET /api/matching/bills/<bill_id>/candidates` 与 `GET /api/matching/candidates?billId=...` 除保留原有 `kind="transfer"` 候选外，还会追加 `kind="investment"` 与 `kind="learning"` 两类 historical recommendation。investment 候选使用稳定 `candidateId=bill:<billId>:investment:<candidateBillId>`，并返回 `billId / score / level / reason / bill{...}`；它沿用同用户、未参与其他 pair、金额绝对值相等且方向相反、来源账户不同、时间落在窗口内的约束，并额外要求正式账单文本仍命中 investment keyword 判定，同时受 `bill_investment_pair_suppressions` 过滤。learning 候选继续使用带版本保护的 `candidateId=bill:<billId>:learning:<ruleId>:<ruleRevision>`，并返回 `ruleId / score / level / reason / recommendedType / summary / suppressed`；其评分逻辑复用现有长期学习 composite rule similarity，对正式账单的 `counterparty / description / payment_method` 做只读即时匹配，不引入新的 pair link 结构。对应写接口 `POST /api/matching/manual-pair` 当前接受请求体 `{ billId, candidateBillId, pairType? }`，其中 `pairType` 默认为 `transfer`，支持 `transfer | investment`；运行时结构 map 为 `POST /api/matching/manual-pair -> api/routes/matching.py::_parse_manual_pair_request() -> BillService.create_manual_transfer_pair() / create_manual_investment_pair() -> Database.create_manual_transfer_pair() / create_manual_investment_pair()`。

### 前端建议

在导入预览和后配对中心中提供：

- 候选配对列表
- 候选理由展示
- 置信度展示
- 确认 / 拒绝 / 手动配对
- 过滤维度：
  - 转账
  - 投资
  - 未确认
  - 低置信度
  - 跨批次

当前桌面端已先落一条单账单 historical matching 消费入口：`EditDialog.vue -> BillMatchingPanel.vue -> services.ts::getMatchingBillCandidates()/getMatchingBillFeedback()/acceptMatchingCandidate()/rejectMatchingCandidate()/deleteMatchingPair() -> GET /api/matching/candidates?billId=... / GET /api/matching/bills/<bill_id>/feedback / POST /api/matching/candidates/<candidate_id>/accept / POST /api/matching/candidates/<candidate_id>/reject / DELETE /api/matching/pairs/<id>`。该入口当前会在已入库交易详情里展示 `linkedPair`、`transfer / investment / learning` 候选、pair feedback 事件流，并支持对历史候选执行 generic accept / reject 以及清除当前 manual pair；当 formal-bill learning candidate accept 返回更新后的 `bill` 时，详情弹窗会同步刷新宿主交易字段；它是单账单详情面板，而不是新的 pairing center 路由。

当前桌面端已落地独立配对中心路由：`MainLayout.vue(nav "配对中心") -> router/desktop.ts(/pairing/list) -> views/desktop/pairingcenter/ListPage.vue -> stores/matching.ts -> services.ts::getMatchingPairs()/deleteMatchingPair()/createManualPair()/reconcileMatchingHistory()`。该页面展示当前用户全部已持久化 manual pair 列表，支持按 pairType 侧栏过滤、删除确认对话框、空状态占位；前端新增 `models/bill_matching.ts` 中的 `MatchingPairsResponse / ReconcileHistoryResponse` 结构与 `normalizeMatchingPairsResponse()` 转换，以及 `stores/matching.ts` composition API store 管理 pairs / loading / error / pairTypeFilter 状态。i18n 已覆盖 en / zh_Hans。

### parser 标签建议

建议 parser 增加统一元标签：

- `parser_id`
- `record_origin`
- `channel`
- `txn_family_hint`
- `account_hint`
- `institution_type`

这些信息优先存到：

- `bills_parser_template` 的 JSON 扩展列中
- 然后写入 `bills_preview`

### 风险

- 如果一开始就做复杂事件组，复杂度会快速上升
- 如果不给出解释，用户不会信任自动配对
- 跨批次配对在 SQLite 上容易性能失控，需要索引和窗口限制

### 验收标准

- 导入预览可看到配对候选与理由
- 用户可手动确认 / 拒绝
- 可对历史账单执行后配对
- 配对结果可回灌学习层
- 可限制在时间窗口和候选池内执行，性能可控

### 优先级

- **高优先级**
- **难度：中高**
- **收益：很高**

---

## 6.4 阶段 3：备份、secret、数据库保护和恢复演练

### 目标

把目前“能备份”升级成“能恢复、能验证、能定时、能加密”。

### 推荐实施顺序

1. 先做备份元信息与校验 **（已完成：2026-03-31，已补 backup_records、checksum、restore verify 与审计）**
2. 再做加密备份 **（已完成：2026-03-31，当前已支持基于环境密钥的本地加密备份与恢复）**
3. 再做定时调度 **（已完成：2026-03-31，已先落地 backup_jobs 配置层与 `/api/backup/jobs` 最小 CRUD）**
4. 再评估 SQLite 文件级加密 / SQLCipher

### 建议新增模块

- `src/bill_analyser/core/backup/backup_service.py`
- `src/bill_analyser/core/backup/restore_service.py`
- `src/bill_analyser/core/backup/retention.py`
- `src/bill_analyser/core/backup/verify_restore.py`
- `src/bill_analyser/core/security/secret_provider.py`
- `src/bill_analyser/core/security/jwt_keyring.py`

### 建议新增表

#### `backup_records`

**（已完成：2026-03-31，当前已落地本地备份记录持久化与状态更新）**

建议字段：

- `id`
- `user_id`
- `backup_name`
- `storage_type` (`local` / `remote`)
- `file_path`
- `checksum`
- `encrypted`
- `backup_scope_json`
- `app_version`
- `schema_version`
- `status`
- `created_at`

#### `backup_jobs`

**（已完成：2026-03-31，当前已落地任务配置与 retention 参数持久化，调度执行器后续补齐）**

建议字段：

- `id`
- `user_id`
- `job_type` (`daily` / `weekly` / `manual`)
- `schedule_expr`
- `retention_days`
- `retention_count`
- `enabled`
- `last_run_at`
- `last_status`
- `created_at`
- `updated_at`

### secret 管理建议

#### 近期方案

- 统一读取顺序：
  1. 运行环境变量
  2. `.env`
  3. `.env.example` 仅作为示例，不参与运行
- `JWT_SECRET_KEY`、操作密码等从环境读取

#### 中期方案

定义 secret provider 抽象：

- `EnvSecretProvider`
- `LocalKeyringSecretProvider`
- `VaultSecretProvider`（可选）

### 数据库保护建议

#### 短中期建议先做

- 敏感字段加密
- 备份文件加密
- 目录 ACL / 磁盘加密文档
- 权限控制放在应用层

#### 长期评估

- SQLCipher
- Vault 集成

### 为什么不建议马上把 SQLite 推翻

因为当前项目的主要价值链在：

- 导入
- 分类
- 配对
- 分析
- 预算

而不是在数据库多租户/复杂事务特性。

直接大改数据库，会把主线目标拖慢，且未必马上带来用户可感知价值。

### 验收标准

- 可以看到备份记录、校验状态、恢复验证结果
- 可以手动触发备份，也可以定时执行
- 备份文件支持加密 **（已完成：2026-03-31，本地备份可选输出 `.zip.enc`）**
- secret 不再通过不安全的方式散落保存

### 优先级

- **高优先级**
- **难度：中高**
- **收益：高**

---

## 6.5 阶段 4：产品化增强（对标竞品但不盲目追大）

### 目标

在核心链路稳定后，补足几个真正有价值的产品能力。

### 建议优先的产品功能

#### 1. 周期账单 / 订阅识别（高优先级） ✅ 已落地

**为什么要做**：

- 这是预算、预测、现金流提醒的前提；
- 同类产品里几乎都是高频能力；
- 当前仓库已有 `recurring_bills` / template 基础，属于高 ROI 增强。

**已实现**：

- 后端：`recurring_detection.py` 模式检测算法（同方向 + 相近金额 + 同 counterparty + 周期间隔 → 置信度评分）
- 持久层：`db_recurring_suggestions.py` mixin + `recurring_suggestions` 表
- API：`/api/recurring/suggestions` (GET 列表 / POST detect / POST accept / POST reject)
- 前端：`DiscoverPage.vue` + `recurring.ts` store + 4 个 services 方法
- 路由：`/recurring/discover`

**难度**：中等

#### 2. 日历 / 现金流视图（高优先级） ✅ 已落地

**为什么要做**：

- 比单纯图表更适合看“什么时候会花/入多少钱”；
- 可以直接结合 recurring 与已入账交易。

**已实现**：

- 后端：`GET /api/calendar/events?start_date&end_date` — 按日聚合已入账交易 + recurring 预测投影
- 前端：`CalendarPage.vue` — 月历网格 + 每日收支 + recurring 标记 + 月度汇总卡片
- 路由：`/calendar`

**难度**：中等

#### 3. 净资产 / 资产负债视图（高优先级） ✅ 已落地

**为什么要做**：

- 你已经有账户、投资、统计和汇率基础；
- 这是非常自然的产品升级方向。

**已实现**：

- 后端：`GET /api/networth/snapshot` — 按账户类型分组聚合资产/负债/净值
- 前端：`NetWorthPage.vue` — 资产/负债分组表格 + 净值三卡片汇总
- 路由：`/networth`

**难度**：中等

#### 4. 规则中心 / 解释中心（高优先级） ✅ 已落地

**为什么要做**：

- AI、长期学习、自动配对都必须给用户一个可解释入口；
- 否则规则越多，系统越难维护。

**已实现**：

- 后端：`GET /api/rules/overview` — 统一聚合 learning rules + category keywords + recurring rules
- 前端：`RuleCenterPage.vue` — 三标签分组展示（学习规则/分类关键词/周期规则）
- 路由：`/rules/center`

**难度**：中等

#### 5. 异常账单 / 风险洞察（中优先级） ✅ 已落地

**为什么要做**：

- 这是提升“每天想打开看看”的重要能力；
- 例如：本月餐饮异常上涨、重复扣款嫌疑、未配对转账等。

**已实现**：

- 后端：`GET /api/insights/anomalies?months` — 三类异常检测（大额交易>3x均值 + 重复扣款嫌疑 + 月度分类偏离>2x）
- 前端：`InsightsPage.vue` — 异常卡片列表 + 严重程度分色 + 时间范围选择
- 路由：`/insights`

**难度**：中等偏高

### 低优先级但可长期考虑的方向

- 银行直连
- 家庭共享 / 协作记账
- 加密资产深度接入
- 双分录全面重构

这些不是没价值，而是目前**不适合作为主线优先事项**。

---

## 7. 重点主题的落地方案

## 7.1 导入模块引入大模型能力

### 目标

根据长期学习样本自动生成关键词候选，经用户确认后加入分类关键词，提升分类准确率和效率。

### 推荐方案

#### 第一阶段：不急着上大模型，先做确定性挖掘

从以下来源抽取高频特征：

- 交易对方
- 描述片段
- 支付方式
- parser tags
- 已确认配对结果

然后归并出候选关键词。

#### 第二阶段：大模型作为“归纳器”而非“最终裁判”

输入：

- 某分类的近期高质量样本摘要
- 候选关键词列表
- 已有规则关键词

输出：

- 建议保留关键词
- 建议新增关键词
- 冲突关键词
- 建议说明

### 推荐提示词思路

LLM 的提示词应强调：

- 输出必须是短关键词 / 短语，不是解释性句子
- 不能输出过于泛化的词
- 优先保守，避免污染规则
- 说明为何建议这个词

### 风险控制

- 发送前脱敏
- 限制样本量
- 限制字段类型
- 默认关闭
- 所有结果走 `pending`

### 代码落点

- `core/import_learning/keyword_miner.py`
- `core/import_learning/suggestion_service.py`
- `core/llm/openai_compatible_provider.py`

---

## 7.2 转账账单与投资账单配对问题

### 目标

提升准确率，并让配对结果反馈给长期学习，支持导入时配对、后配对、跨批次配对。

### 推荐方案

#### 统一做成“候选 + 评分 + 解释 + 状态机”

候选生成考虑以下维度：

- 时间窗口
- 金额关系
- 收支方向
- 源/目标账户关系
- parser tags
- payment_method / counterparty / description
- 历史成功样本

#### 推荐评分要素

- 金额完全匹配：高分
- 时间窗口很近：高分
- parser family 互补：高分
- 同类场景历史成功：高分
- 存在冲突标签：减分

#### 结果状态建议

- `pending`
- `accepted`
- `rejected`
- `expired`
- `merged`

### 后配对功能建议

在 UI 中提供：

- 从未配对账单中选择两条或多条记录
- 系统预填配对理由与建议方向
- 用户确认后落正式关系
- 同时生成学习反馈事件

### 回灌学习建议

配对确认后，可以将以下信息作为长期学习样本：

- parser_id 组合
- payment_method 组合
- counterparty / description 模式
- source/destination account pattern

用于下一次生成：

- 配对建议
- 分类建议
- 账户匹配建议

### 代码落点

- `core/matching/*`
- `core/import_learning/feedback_promoter.py`
- `api/routes/matching.py`

---

## 7.3 验证功能、2FA、jwt_secret

### 当前判断

不是“没有”，而是“要做完”。

### 建议拆分

#### 认证层闭环

- 登录后若启用 2FA，不得直接签发正式 session token
- `pending_2fa` token 单独控制生命周期
- verify 成功后才创建正式 session

#### 恢复码治理

- 恢复码只展示一次
- 数据库存哈希，不存明文 **（已完成：2026-03-30）**
- regenerate 时旧的全部失效 **（已完成：2026-03-30）**

#### secret 管理

- `JWT_SECRET_KEY` 外置到环境变量
- `JWT_SECRET_KEY` 外置到环境变量 **（已完成：2026-03-30）**
- 本地 `.env` 用占位值，生产必须替换 **（已完成：2026-03-30）**
- 后续加入 keyring / vault adapter

#### step-up auth

建议对以下敏感操作增加二次验证：

- 关闭 2FA
- 重新生成恢复码
- 执行恢复备份
- 导出全量敏感数据
- 清空数据

### 代码落点

- `api/routes/auth.py`
- `core/security/*`

---

## 7.4 数据库、加密、vault、权限控制

### 建议先做的现实路线

#### 第一层：应用权限控制

SQLite 本身不适合数据库内建 RBAC，建议：

- 在 API 层补强认证与授权
- 控制敏感接口、敏感操作、敏感导出

#### 第二层：敏感字段加密

可以优先对以下数据做字段级加密：

- 恢复码哈希/密钥材料
- 敏感配置
- 未来可能的第三方 token

#### 第三层：备份加密

比“全库透明加密”更容易落地，也更快产生实际安全收益。

#### 第四层：评估 SQLCipher

只有在以下条件成立时再推进：

- Windows 环境兼容测试通过
- `aiosqlite` 接入方案稳定
- 备份/恢复/升级流程明确

### 对 Vault 的建议

Vault 很适合作为“部署版增强方案”，但不建议当前作为本地单机版本的必选前置条件。

建议设计成 provider：

- 本地模式：env / keyring
- 部署模式：vault

这样不会拖慢当前主线。

---

## 7.5 加一个 skill，让新增解析器成为标准流程

**（已完成：2026-04-08，已新增 `.agents/skills/add-parser-standard-flow/SKILL.md` 与 `docs/parsers/add-parser-standard-flow.md`，并纳入 AI 工作流与健康检查回归）**

### 建议结论

这个非常值得做，而且应该做成**共享 skill**，而不是只写一篇文档。

### 推荐落点

- `.agents/skills/add-parser-standard-flow/SKILL.md`
- 可选补充：`.github/prompts/add-parser-standard-flow.prompt.md`
- 说明文档：`docs/parsers/add-parser-standard-flow.md`

### skill 应覆盖的标准步骤

1. 明确账单来源与样本范围
2. 列出现有 parser 的冲突对象
3. 设计检测条件与排除条件
4. 明确优先级与 `ParserFactory` 中的选择规则
5. 标准输出字段与 parser tags
6. 增加正例、负例、歧义例测试
7. 更新 manifest / fixture
8. 更新 `docs/PROJECT_OVERVIEW.md`
9. 补充维护说明与失败案例

### 必须强调的工程规则

- 不允许只写正例测试
- 必须有误判负例
- parser 不能自由发挥输出字段
- parser tags 必须使用受控词表
- 新 parser 合入前必须证明不会误伤已有 parser

### 优先级

- **高优先级**
- **难度：低到中**
- **收益：高**

---

## 8. 对比同类产品，建议新增但你原文未明确写出的功能

下面按优先级列出建议。

## 8.1 高优先级

### 1. 周期账单 / 订阅管理

**为什么需要**：

- 周期支出是个人财务管理的核心场景；
- 能显著提升预算、提醒、现金流预测的价值；
- 同类产品普遍具备。

**实现难度**：中等

**建议优先级**：高

### 2. 日历 / 现金流视图

**为什么需要**：

- 用户不只关心“本月花了多少”，更关心“这周/下周会发生什么”；
- 与 recurring、预算、未配对交易、未来提醒天然联动。

**实现难度**：中等

**建议优先级**：高

### 3. 净资产 / 资产负债视图

**为什么需要**：

- 你已经有账户、投资、汇率、统计基础；
- 这是非常自然且很有感知价值的升级方向。

**实现难度**：中等

**建议优先级**：高

### 4. 规则中心 / 解释中心

**为什么需要**：

- 学习规则、配对规则、分类规则会越来越多；
- 没有规则中心，后续维护会越来越痛苦；
- 也是 AI 功能赢得用户信任的关键界面。

**实现难度**：中等

**建议优先级**：高

### 5. 安全中心

**为什么需要**：

- 账单与资产是高敏数据；
- 用户需要可见的安全设置、备份设置、恢复码管理与审计信息。

**实现难度**：中等

**建议优先级**：高

## 8.2 中优先级

### 6. 异常交易 / 洞察提醒

**为什么需要**：

- 增强“每天打开看一眼”的理由；
- 有利于从工具走向助手。

**实现难度**：中高

**建议优先级**：中

### 7. 多币种资产估值与汇率追踪增强

**为什么需要**：

- 当前已有汇率能力；
- 往净资产视图延伸会很自然。

**实现难度**：中等

**建议优先级**：中

### 8. API token / 外部集成中心

**为什么需要**：

- 适合导出、脚本、自动化和后续生态扩展；
- 当前已经有 token/session 基础。

**实现难度**：中等

**建议优先级**：中

### 9. 学习规则版本化 / 回滚

**为什么需要**：

- 学习规则一多，没有版本管理就会很难排查污染；
- 也是 AI 能力工程化的基础。

**实现难度**：中等

**建议优先级**：中

## 8.3 低优先级

### 10. 银行直连 / 自动同步

**为什么需要**：

- 价值很高，但对接、维护、合规、稳定性成本都很高。

**实现难度**：高

**建议优先级**：低

### 11. 家庭共享 / 协作记账

**为什么需要**：

- 对部分用户有价值，但不是当前主线核心。

**实现难度**：高

**建议优先级**：低

### 12. 全量双分录重构

**为什么需要**：

- 从理论上很强，但会触动整个模型和导入链。

**实现难度**：很高

**建议优先级**：低

---

## 9. 推荐 API 规划（建议，不要求一次性全部实现）

## 9.1 学习建议相关

- `GET /api/bills/import/v2/learning/<session_id>/suggestions`
- `POST /api/bills/import/v2/learning/<session_id>/suggestions`（当前已支持 `preview_updates[]` 与可选 `previewIds[]`，统一收口到 route -> bill_service -> db_import_learning 的 session annotation save + filtered dry-run suggestions 链路）
- `POST /api/bills/import/v2/learning/<session_id>/promote`（当前已支持 `preview_updates[]` 与可选 `previewIds[]` 双入口，统一收口到 route -> bill_service -> db_import_learning 的 `preview_ids` 提升链路；显式空 `previewIds[]` 返回零提升）
- `GET /api/learning/suggestions`
- `POST /api/learning/suggestions/generate`
- `POST /api/learning/suggestions/<id>/accept`
- `POST /api/learning/suggestions/<id>/reject`
- `POST /api/learning/suggestions/batch-accept`

## 9.2 配对相关

- 当前 matching 域已提供 `GET /api/matching/sessions/<session_id>/candidates`：返回 `session_id`、`summary(preview_count/candidate_count/counts_by_kind)` 与按 `preview:<preview_id>:<kind>` 稳定编号的 `candidates[]`；单条 candidate 结构包括 `kind`、`score/level/reason/status`、`details`、`preview` 快照，以及 `context(dedup/parser/annotation)`。
- `GET /api/matching/candidates`
- `POST /api/matching/candidates/<id>/accept`
- `POST /api/matching/candidates/<id>/reject`
- `POST /api/matching/candidates/<id>/clear`
- `POST /api/matching/manual-pair`（请求体为 `{ billId, candidateBillId, pairType? }`，`pairType` 默认 `transfer`，当前支持 `transfer | investment`）
- `POST /api/matching/reconcile-history`（请求体 `{ billIds[], families?[] }`，`families` 允许 `transfer / investment / learning`，默认 `["transfer"]`）
- `GET /api/matching/pairs`
- `DELETE /api/matching/pairs/<id>`

## 9.3 安全中心相关

- `GET /api/security/overview`
- `POST /api/security/step-up/verify`
- `POST /api/security/secret/rotate`（管理员 / 本地模式谨慎提供）

## 9.4 备份相关

- `GET /api/backup/jobs`
- `POST /api/backup/jobs`
- `POST /api/backup/run`
- `POST /api/backup/restore/verify`
- `POST /api/backup/restore`

---

## 10. 推荐测试策略

## 10.1 配对与学习增强的测试重点

### 后端测试

- 正例
- 负例
- 歧义例
- 低置信度例
- 跨批次配对性能边界
- suggestion accept/reject 回放测试

### 前端测试

- 候选展示
- 用户确认 / 拒绝 / 编辑后接受
- 导入预览交互不会因建议变化导致行消失或状态错乱

### 验收门槛建议

如果未来改动进入 `src/bill_analyser/**` 运行时代码：

- 先跑相关 pytest
- 业务代码验收前仍要跑全量：
  - `./.venv/Scripts/python.exe -m pytest tests/ -v`

如果改动进入前端：

- 至少跑 `npm run lint`
- 必要时做最小构建验证

## 10.2 parser 新增的测试标准

每新增一个 parser，至少要有：

- 正例样本
- 负例样本
- 与相似 parser 的歧义样本
- `ParserFactory` 顺序/优先级测试
- 输出字段契约测试
- parser tags 契约测试

---

## 11. 实施顺序建议（最关键）

建议按下面顺序推进。

## 第一优先：安全与恢复

1. `.env` / secret 外置基线
2. 2FA 真闭环
3. 恢复码哈希化
4. 备份校验与恢复预验证 **（已完成：2026-03-31）**

**原因**：这部分风险最低，但对真实可用性和可信度提升最大。

## 第二优先：parser tags + 配对域骨架

1. 标准化 parser tags **（已完成后端契约 MVP：2026-04-08；当前已打通 parser 输出、导入临时表持久化与预览/导入 API 返回）**
2. 建立 `matching/*` 目录和候选/评分/解释结构 **（已完成只读结构 MVP：当前新增 `src/bill_analyser/core/matching/*`，`BillService.get_import_preview()` / `/api/bills/import/v2/dedup` 的 `preview[]` 项会额外返回 `matching.transfer|investment|learning|recurring|dedup|parser|annotation` 结构；保留原有平铺 suggestion/signals 字段不变）**
3. 导入预览展示配对候选 **（已完成前端消费闭环 MVP：`Check Data` 类型列开始优先消费 `preview[].matching.*`；`transfer / investment / learning / recurring` 继续沿用候选 chips，`parser / dedup / annotation` 作为补充上下文摘要；`confirm` / `reclassify` 写入结构保持不变）**。其中 `Learning Suggestion` 当前已从只读提示扩成 `accept / reject / clear` 闭环：运行时结构 map 为 `ImportTransactionCheckDataTab.vue -> checkDataCandidateReview.ts::buildImportCheckLearningDecisionExpectedState() -> services.ts::acceptMatchingCandidate()/rejectMatchingCandidate()/clearMatchingCandidate() -> POST /api/matching/candidates/<candidate_id>/accept|reject|clear`。learning candidate id 形如 `preview:<preview_id>:learning`，前端会把 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId, sourceAccountId, destinationAccountId)` 与 `ruleId`（accept 时）一起下发；成功后复用刷新后的 `preview[]` 回写当前行的类型 / 分类 / 账户 / learning 状态。`Investment Signal` 当前也已从只读提示扩成 `accept / reject / clear` 审查入口：运行时结构 map 为 `ImportTransactionCheckDataTab.vue -> checkDataCandidateReview.ts::buildImportCheckDecisionExpectedState() -> services.ts::acceptMatchingCandidate()/rejectMatchingCandidate()/clearMatchingCandidate() -> POST /api/matching/candidates/<candidate_id>/accept|reject|clear -> api/routes/matching.py -> BillService.apply_preview_investment_decision(...)`。其中 investment candidate id 形如 `preview:<preview_id>:investment`，前端会下发 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)`；成功后复用刷新后的 `preview[]` 回写当前行的 investment review 状态，但该写路径只更新 `preview_matching_feedback_json.investment{review_status, suppressed}`，不直接改写 `preview_type / category / recurring` 业务字段。
4. 导入预览当前已提供 `POST /api/bills/import/v2/preview-item/<preview_id>/transfer-decision` 处理转账建议决策；请求体使用 `decision=accept|reject|clear`，并额外携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)` 作为防陈旧写保护；若接口发现预览状态已变化，则返回 `409 Preview state changed, please refresh`，否则响应返回刷新后的整批 `preview[]`。对应结构中，`matching.transfer` 在候选分数字段之外，额外承载 `review_status / reviewed_type / suppressed`，用于表达 pending / accepted / rejected 决策状态，并支持在 `clear` 时恢复 accept 前的 `preview_type / category / recurring` 展示 map。当前 `reclassify` / `confirm` 提交的 `preview_updates[]` 会在本地手工类型/分类/周期编辑与既有 transfer 决策发生冲突时，追加 `clear_transfer_decision=true`，用于清除已持久化的 transfer 决策反馈，保证手工编辑后的结构 map 与决策状态一致。
5. 导入预览当前已提供 `PUT /api/bills/import/v2/preview-item/<preview_id>/recurring-match` 与 `DELETE /api/bills/import/v2/preview-item/<preview_id>/recurring-match` 处理单条定时候选确认/清除；两者都要求 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)` 作为防陈旧写保护，其中 `PUT` 额外要求 `recurringId`。接口成功后同样返回刷新后的整批 `preview[]`；它只更新 `bills_preview.preview_recurring_*` 与相关 `matching.recurring` 展示字段，并会清除与手工周期决策冲突的 transfer review feedback，但不会在 preview 阶段提前推进 `recurring_bills.next_date`。当前 `Check Data` 的单条 `Choose Scheduled Match` / `Clear Scheduled Match` 已改为直接走这组接口，而不是仅停留在前端本地草稿。
6. matching 域当前已额外提供 `GET /api/matching/sessions/<session_id>/candidates` 只读出口：它将当前 session 下每条 `preview.matching.transfer|investment|learning|recurring` 投影为显式 `candidates[]`，并把 `dedup/parser/annotation` 作为 `context` 附着到每条 candidate；该接口当前只负责会话级读模型，不新增数据库表，也不承载 accept/reject/manual-pair 写语义。其中 `recurring` candidate 在 `preview_recurring_id` 已写入时会投影为 `status=confirmed`，而清除绑定但仍保留候选数量时会继续投影为 `status=pending`。
7. matching 域当前已进一步提供历史正式账单后配对 MVP：`GET /api/matching/bills/<bill_id>/candidates` 会按当前用户读取单条正式账单，并返回 `data{billId, linkedPair, candidates[]}`；其中 `linkedPair` 在账单已配对时包含 `id / pairType / source / leftBillId / rightBillId / otherBillId`，未配对时为 `null`，而 `candidates[]` 会同时投影 `kind="transfer"`、`kind="investment"` 与 `kind="learning"` 三类历史候选。transfer 候选继续要求同用户、未参与其他 pair、金额绝对值相等且方向相反、来源账户不同、且时间落在窗口内，并为每项返回 `billId / score / level / reason / bill{...}`；investment 候选沿用同一组金额/方向/账户/时间窗口约束，并额外要求正式账单文本仍命中 investment keyword 判定；learning 候选则返回 `ruleId / score / level / reason / recommendedType / summary / suppressed`。`GET /api/matching/bills/<bill_id>/feedback` 当前会按 bill 返回 `data{billId, events[]}`，把 `bill_pair_feedback` 中与该 bill 相关的 append-only formal-bill feedback 事件投影为 bill-scoped 审查流；每条 event 至少包含 `id / candidateId / action / createdAt / payload`，并按最新事件优先排序。`POST /api/matching/manual-pair` 当前使用请求体 `{ billId, candidateBillId, pairType? }` 建立 1:1 manual pair，其中 `pairType` 默认为 `transfer`，支持 `transfer | investment`；运行时结构 map 为 `POST /api/matching/manual-pair -> api/routes/matching.py::_parse_manual_pair_request() -> BillService.create_manual_transfer_pair() / create_manual_investment_pair() -> Database.create_manual_transfer_pair() / create_manual_investment_pair()`。`self-pair` 返回 `400`，账单不存在或不属于当前用户返回 `404`；当 `pairType=transfer` 时，重复/反向重复/任一账单已参与其他 transfer pair、或两条账单不满足 transfer-only 配对约束时返回 `409`；当 `pairType=investment` 时，冲突与候选约束则复用 investment/manual pair 规则并返回同样的 `409`。`POST /api/matching/candidates/<id>/accept` 当前已可将 historical transfer/investment candidate 的成功写路径回灌为 `bill_pair_feedback` 事件：transfer accept 会记录最小 transfer/manual pair payload，investment accept 会记录最小 investment/manual pair payload。`POST /api/matching/candidates/<id>/reject` 当前也会把 historical transfer/investment generic reject 结果写为 `bill_pair_feedback(action="reject")` 事件。`DELETE /api/matching/pairs/<id>` 也已可删除当前用户下的 manual pair（包括 investment/manual）。该 MVP 当前以 `bill_pair_links` + `bill_pair_feedback` 作为正式账单 pair/result 的最小持久化模型，同时以 `bill_investment_pair_suppressions` 与 `bill_learning_rule_suppressions` 作为 formal-bill investment / learning reject 的最小持久化模型；`GET /api/matching/pairs` 当前会读取 `bill_pair_links` 的 manual 子集，并至少暴露 `transfer/manual` 与 `investment/manual` 两类 pair，formal-bill learning accept 则继续直接写回 `bills` 与 `import_learning_rule_logs`，不引入新的 learning pair/link 结构。相关 pair link 与 suppressions 会在单条删除、批量删除、清空用户交易、账户级删交易与清空用户业务数据时同步清理，而 `bill_pair_feedback` 当前保持为 append-only event stream，并通过 `GET /api/matching/bills/<bill_id>/feedback` 提供 bill-scoped 审查视图，但不参与 pair list 聚合或这些清理路径。

**原因**：这是提升导入质量最直接的主线。

## 第三优先：长期学习建议层

1. 先做确定性关键词挖掘
2. 再做 suggestion center
3. 再接 LLM provider

**原因**：先把闭环跑通，再让 AI 来放大效果，风险最小。

## 第四优先：备份加密、调度和密钥 provider

1. backup records
2. backup encryption
3. scheduler
4. secret provider abstraction

**原因**：是安全与运维增强，但优先级略低于前面的主体验改善。

## 第五优先：产品化能力补齐

1. 周期账单 / 订阅
2. 日历 / 现金流
3. 净资产
4. 规则中心
5. 异常洞察

**原因**：这部分最适合在底层可信之后做，效果会更稳。

---

## 12. 最建议的 3 个近期落地切片

如果要快速见效果，我最建议优先做以下 3 个切片：

### 切片 A：安全最小闭环

范围：

- 2FA 完整登录闭环
- 恢复码哈希化 **（已完成：2026-03-30，含一次性消费与旧码失效）**
- `JWT_SECRET_KEY` 外置 **（已完成：2026-03-30）**
- 敏感操作二次验证基础 **（已完成：2026-03-31，已支持通过 `/api/security/step-up/verify` 签发短期 step-up token，并用于 2FA 禁用/恢复码重生成/数据清理等敏感操作）**

**价值**：高，且几乎不需要碰导入主链。

### 切片 B：后配对 + 跨批次配对 MVP

范围：

- 新增配对候选接口
- 导入预览可确认候选
- 支持历史账单后配对
- 结果写回并生成反馈事件

**价值**：非常高，直接改善导入体验。

### 切片 C：学习建议中心 MVP（先无 LLM）

范围：

- 从人工修正中提取候选关键词
- suggestion pending / accept / reject
- accept 后写入 `import_learning_rules`

**价值**：高，而且能为未来的 LLM 能力提供非常好的落点。

---

## 13. 最终建议总结

### 13.1 这条路线为什么适合当前项目

因为它：

- 尊重现有代码结构；
- 不要求重做主架构；
- 能逐步提升导入质量；
- 能把 AI 放在安全、可解释的位置；
- 能在工程上控制风险；
- 能把安全与恢复能力补起来；
- 能为后续产品化能力打基础。

### 13.2 最核心的战略判断

Bill Analyser 下一阶段最应该做的，不是“再堆一堆功能”，而是优先形成这四个闭环：

1. **导入闭环**：解析 -> 去重 -> 配对 -> 分类 -> 预览 -> 确认
2. **学习闭环**：人工修正/配对结果 -> suggestion -> 用户确认 -> 规则沉淀 -> 回放
3. **安全闭环**：登录 -> 2FA -> 敏感操作校验 -> 审计 -> secret 管理
4. **恢复闭环**：备份 -> 校验 -> 保留 -> 恢复预演 -> 恢复 **（已补齐到 retention 收敛层：2026-03-31，保留策略现已覆盖 `.zip.enc` 且删除会同步更新备份记录状态）**

这四个闭环打通之后，再去加：

- LLM 增强
- 周期账单
- 日历
- 净资产
- 洞察中心

会更稳，也更有产品说服力。

---

## 14. 推荐优先级总表

| 主题 | 当前基础 | 建议优先级 | 难度 | 说明 |
|---|---|---:|---:|---|
| 2FA 真闭环 + secret 外置 | 有基础 | 最高 | 中 | 安全最先补 |
| 备份校验 + 恢复预验证 | 有基础 | 高 | 中 | 数据安全刚需 |
| parser tags 标准化 | 有基础 | 高 | 中 | **已完成后端契约 MVP（2026-04-08）**；现为配对/学习/分类共用底座 |
| 转账/投资配对域收口 | 有基础 | 高 | 中高 | 导入体验最值钱的主线 |
| 后配对 / 跨批次配对 | 有基础 | 高 | 中高 | 用户强需求 |
| 学习建议中心（无 LLM 版） | 有基础 | 高 | 中 | 闭环先跑通 |
| LLM 候选归纳器 | 需新增 provider | 中高 | 中 | 放在建议层，不直接决策 |
| 备份加密 / 定时调度 | 有基础 | 中高 | 中高 | 运维增强 |
| parser 新增 skill | 有基础 | 高 | 低中 | 低风险高收益 |
| 周期账单 / 订阅 | 有部分基础 | 高 | 中 | ✅ 已落地：/api/recurring/* |
| 日历 / 现金流视图 | 需新 UI | 高 | 中 | ✅ 已落地：/api/calendar/events |
| 净资产 / 资产负债 | 有账户基础 | 高 | 中 | ✅ 已落地：/api/networth/snapshot |
| 规则中心 / 解释中心 | 需新增 UI/接口 | 高 | 中 | ✅ 已落地：/api/rules/overview |
| 异常洞察 | 有统计基础 | 中 | 中高 | ✅ 已落地：/api/insights/anomalies |
| 银行直连 | 无基础 | 低 | 高 | 暂不建议做主线 |
| 协作 / 家庭共享 | 基础不足 | 低 | 高 | 非当前主线 |
| SQLCipher 全库加密 | 无成熟基线 | 低中 | 高 | 先评估，别抢主线 |

---

## 15. 建议同步补的文档与流程资产

除了代码，还建议同步准备：

- `docs/import/learning-and-pairing.md`
- `docs/security/auth-secrets-backup.md`
- `docs/parsers/add-parser-standard-flow.md`
- `.agents/skills/add-parser-standard-flow/SKILL.md`
- 更新 `docs/PROJECT_OVERVIEW.md` 中稳定业务事实

这样后续你每推进一步，不只是“代码多了”，而是系统的工程秩序也一起建立起来。

---

## 16. 一句话收尾

如果只保留一句最重要的建议，那就是：

> **Bill Analyser 现在最值得走的路线，是在现有导入主链上新增“可解释的建议层”和“可确认的学习闭环”，同时优先补齐安全与恢复能力；先把系统做得更可信、更稳，再把它做得更聪明。**
