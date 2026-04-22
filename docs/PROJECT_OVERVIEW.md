# Bill Analyser 项目总览（当前代码基线）

## 1. 项目定位
Bill Analyser 是一个“多来源账单导入 + 智能去重 + 自动分类 + 多维统计分析”的全栈系统，核心目标是统一管理微信、支付宝与多家银行账单，并提供可编辑、可审计、可扩展的数据处理链路。

---

## 2. 总体架构

### 2.1 分层结构
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

### 2.2 关键架构模式
- **异步桥接模式**：Flask 路由内创建独立事件循环调用 async 逻辑
- **REST 主链模式**：当前运行态主链统一收口到 REST（`/api/...`）
- **适配器/转换模式**：前后端字段、时间、金额单位统一转换

补充说明（2026-03-07）：
- 当前运行态已无 `/api/v1/*` 路由，也无 WSGI 级 URL rewrite 中间件。
- legacy 兼容已退出运行时代码树，当前仅残留在历史快照目录与针对遗留路径的回归测试约束中，不再体现在运行态路由表或适配器实现中。

---

## 3. 后端模块分布

### 3.1 API 路由模块（`src/bill_analyser/api/routes/`）
- `auth.py`：登录、鉴权、用户资料
- `bills.py`：账单 CRUD、导入、预览、确认、批量操作
- `matching.py`：导入会话级 matching candidate 投影、preview family generic accept/reject、历史正式账单后配对与 pair 列表接口，以及配对中心 investment settings 的 canonical REST 读写接口
- `rules.py`：规则中心 legacy 兼容聚合口；当前 overview 只聚合 learning rules、category rule count 与 recurring rules，不再把 legacy `category_keywords` 作为运行时总览来源
- `accounts.py`：账户管理、余额同步
- `categories.py`：分类管理、规则维护
- `category_rules.py`：分类规则的 canonical CRUD / migrate / test 接口；category rules 是当前分类规则体系的正式入口
- `tags.py`：标签管理与关联
- `budgets.py`：预算 CRUD、执行统计、导入导出
- `statistics.py`：统计总览、趋势、汇率
- `templates.py`：模板管理
- `backup.py`：备份相关接口

### 3.1.1 API 适配器出口（`src/bill_analyser/api/adapters/`）
- 中性实现模块：`transaction_adapter.py`、`account_adapter.py`、`category_adapter.py`
- legacy `v1_*` wrapper 文件已移除，避免新增代码继续耦合旧文件名
- 当前真实实现统一收口到中性模块；新增代码、路由与测试应只直接引用这些中性模块
- 静态回归：`tests/new_ui/test_no_direct_legacy_adapter_usage.py` 会阻止重新引入 `V1*Adapter` / `V1ResponseBuilder` 旧命名、直接导入 `v1_* adapter` 模块，或重新提交这些 wrapper 文件

### 3.2 核心业务模块（`src/bill_analyser/core/`）
- `db.py`：薄 `Database` façade；对外维持统一导入入口，内部按 mixin 组装数据库能力
- `db_runtime.py` / `db_shared.py` / `db_time.py`：数据库连接生命周期、共享请求/分组数据结构、UTC 时间与缓存辅助
- `db_schema.py` + `db_schema_core.py` + `db_schema_users_security.py` + `db_schema_templates_imports.py`：schema 初始化与迁移编排；核心业务表、用户安全表、模板/导入相关表分别维护
- `db_bills.py` / `db_categories.py` / `db_accounts.py` / `db_tags.py` / `db_templates.py`：账单、分类、账户、标签、模板域的 CRUD、批量操作与查询辅助
- `db_users_auth.py` / `db_user_data.py` / `db_audit_backup.py`：用户与会话、2FA 与应用设置、用户数据管理、审计/备份域持久化逻辑
- `db_budgets_core.py` / `db_budgets_execution.py` / `db_budgets_forecast.py`：预算主数据、分类上下文与分组 helper，以及执行统计 / 历史 / 预测 / 导入导出查询
- `db_budgets_reporting.py`：预算 reporting 兼容聚合层；运行时通过它组合 execution/history 与 forecast/import/export 两个预算域 mixin
- `db_import_configs.py` / `db_import_sessions.py` / `db_import_preview.py` / `db_import_learning.py`：导入模板配置、三阶段会话、预览编辑/确认、长期学习规则、session-scoped dry-run learning suggestions 与复合匹配特征
- `bill_service.py`：导入主流程编排（含 v2 三阶段导入）
- `smart_dedup.py`：智能去重引擎（转账配对、平台银行去重、相似去重、分账去重）
- `category_engine.py`：分类规则表达式解析与分类匹配（含类型过滤与预编译优化）；运行时仅从 `category_rules` canonical source 加载规则，不再回退到 `categories.keywords`
- `exchange_rate_providers.py`：多汇率提供者聚合
- `budget.py`：历史预算管理兼容层，保留 `BudgetManager` 供旧 CLI / 旧测试路径复用；当前 CLI 预算报告主链直接走 `Database.get_budget_execution_details()`
- `sync.py`：同步相关编排
- `analyzer.py`：统计分析、聚合与图表/报表数据生成主服务，仍是统计域的活跃运行时代码
- `report.py` / `utils/report_export.py`：`core.report` 保留旧导入路径兼容壳，真实 PDF / Excel / HTML 报告导出实现已归并到 `utils.report_export`
- `smart_dedup.py`：智能去重引擎主实现，仍在导入主链中承担平台银行配对、相似去重与分账去重
- `smart_dedup_v641_backup.py` 等历史版本备份文件已从运行时代码树移除

### 3.2.1 Database façade 关系
- 外部调用方（API 路由、服务、测试）继续只从 `src/bill_analyser/core/db.py` 导入 `Database`
- `Database` 通过多继承顺序组合各域 mixin：预算 reporting → 导入 preview / session / config / learning → 用户数据 / 认证 → 模板 / 标签 / 账户 / 分类 / 账单 → 审计备份 → schema → runtime
- schema 初始化由 `DatabaseSchemaMixin.init_db()` 统一编排，并在运行时通过 `DatabaseRuntimeMixin` 提供连接、路径重定向、缓存与上下文管理
- 这种结构保持了公共 API 稳定，同时把预算域、导入链路、账户/标签/模板、认证安全等高耦合逻辑拆到独立文件维护

### 3.3 解析器模块（`src/bill_analyser/parsers/`）
- 已有解析器：`wechat.py`、`alipay.py`、`icbc.py`、`abc.py`、`ccb.py`、`cmbc.py`
- 工厂入口：`factory.py`（自动识别并分发）
- `ParserBase` 当前除标准账单字段外，还会生成扁平 `parser_tags` 标签数组；三阶段导入临时表分别通过 `bills_parser_template.parser_tags_json` 与 `bills_preview.preview_parser_tags_json` 持久化这些解析器元标签
- 历史 `parsers_old/` 目录与 `*_parser.py` / `csv_parser.py` / `excel_parser.py` 兼容 shim 已移除；运行态仅保留当前工厂与现行解析器实现
- 外部脚本如果仍引用旧 shim 导入路径，应迁移到 `factory.py` 或对应的现行 parser 模块
- 标准解析样本统一放在 `tests/fixtures/import_samples/`，供 parser 单测与真实样本回归复用

---

## 4. 前端模块分布

### 4.1 视图（`src/web/src/views/`）
- `desktop/`：桌面主界面（账户、交易、统计、预算、分类、标签、模板、用户设置）
- `mobile/`：移动端界面（交易、账户、统计、设置等）
- `base/`：公共页面基础

### 4.2 状态管理（`src/web/src/stores/`）
- 业务 store：`transaction.ts`、`account.ts`、`statistics.ts`、`budget.ts`、`transactionCategory.ts`、`transactionTag.ts` 等
- 用户与安全：`user.ts`、`token.ts`、`twoFactorAuth.ts`

### 4.3 服务层（`src/web/src/lib/services.ts`）
- 统一 axios 请求、鉴权头注入、401 刷新 token、REST 主链调用封装

---

## 5. API 对接关系（核心）

### 5.1 Flask 蓝图注册（`src/bill_analyser/api/app.py`）
- RESTful 蓝图：
  - `/api/bills`
  - `/api/accounts`
  - `/api/categories`
  - `/api/matching`
  - `/api/statistics`
  - `/api/tags`
  - `/api/templates`
  - `/api/budgets`
  - `/api/backup`

### 5.1.1 当前 REST 收口进展（2026-03-06）
- 账户域首批 legacy action 已收口到 REST：
  - `PUT /api/accounts/<id>` 支持 `hidden` 可见性更新
  - `PUT /api/accounts/display-orders` 支持批量排序
  - 子账户删除统一通过 `DELETE /api/accounts/<id>`
  - `POST /api/accounts/<id>/transactions/move` 支持账户间批量迁移交易
  - `POST /api/accounts/<id>/transactions/clear` 支持按账户清空交易
- 标签域首批 legacy action 已收口到 REST：
  - `PUT /api/tags/<id>` 同时承担内容更新与 `hidden` 可见性更新
  - `POST /api/tags/batch` 支持批量创建
  - `PUT /api/tags/display-orders` 支持批量排序
- 前端 `services.ts` 与标签 store 已移除对应直连 v1 排序路径，改为统一走 REST 入口
- 模板域已完成契约梳理，但被重新评估为非低风险域：
  - 该域已完成首轮收口：前端模板调用统一走 REST，后端补齐 `templateType` 过滤、双表 DTO 映射、隐藏/排序与 `user_id` 收口；
  - 模板 rewrite 映射、`templates.bp_v1` 注册与实现均已移除；
  - 模板域当前仅保留 REST 主链路与对应回归测试。
- 预算域主链已完成 REST 收口：
  - 前端 `services.ts` 已将列表/详情/创建/更新/删除/执行统计/预测/导入导出统一切换到 `/api/budgets/*`；
  - 服务层新增预算 REST ↔ 前端旧结构兼容映射，避免直接重写 `budget store` 与页面；
  - `db.py` 已为预算 `forecast/import/export` 补齐 `user_id` 隔离；
  - `app.py` 已移除 `budgets.bp_v1` 注册；
  - `src/bill_analyser/api/routes/budgets.py` 内预算旧 `bp_v1` 实现已完成物理删除；
  - 已补 `tests/new_ui/test_budgets_rest_api.py` 覆盖 REST 主链与 legacy 404 回归。
- 账户域历史 rewrite 与旧 `/get` `/modify` `/hide` `/delete` `/move` 兼容路由已移除。
- 标签域 `bp_v1` 注册与全部 v1 兼容实现已移除。
- 账单/分类/账户路由层已完成一轮适配器收敛：
  - `bills.py` 已拆分基础上下文与 adapter 上下文，非转换型路由不再默认构造交易适配器；
  - `accounts.py` / `categories.py` / `bills.py` 已统一改从中性适配器模块导入。

### 5.2 导入链路（重点）
- **v2 三阶段导入**（推荐）
  1. `POST /api/bills/import/v2/parse`
  2. `POST /api/bills/import/v2/dedup`
  3. `POST /api/bills/import/v2/confirm`
  - 配套：session 查询、预览查询与更新、reclassify
- **legacy 导入兼容说明**
  - 前端导入弹窗已不再依赖旧 `v1/transactions/import/process.json` 轮询与 `v1/transactions/parse_dsv_file.json`
  - `v1/transactions/parse_import.json` rewrite 已移除，当前应返回 404

### 5.2.1 导入预览校验交互
- 导入弹窗 `Check Data` 步骤的右上角筛选入口使用分组下拉菜单，按日期、类型、分类、账户、标签、人工标注状态与备注等维度组织筛选项。
- `/api/bills/parse_import` 返回的导入预览项当前会同时携带 `parserSource` 与 `parserTags`；`BillService.get_import_preview()` / `/api/bills/import/v2/dedup` 预览数据会返回 `preview_parser_id` 与 `preview_parser_tags`，而 `/api/bills/import/v2/preview/<session_id>` 的简化结果也会附带 `parserSource` 与 `parserTags` 供前端保留与消费解析器元信息。
- `BillService.get_import_preview()` / `/api/bills/import/v2/dedup` 当前还会为每条预览账单附带嵌套 `matching` 结构，按 `transfer / investment / learning / recurring / dedup / parser / annotation` 分组镜像现有平铺推荐字段；前端导入模型会保留该结构，现有平铺字段语义保持不变。
- 导入学习域当前提供同一路径下的 suggestions 读/预览双入口：`GET /api/bills/import/v2/learning/<session_id>/suggestions` 会直接读取当前用户在该导入会话下的 `import_annotation_samples` 与 `bills_preview`，按 parser-aware `composite` 特征（`parser_id / counterparty / description / payment_method`）聚合同结果样本，并保守过滤两类条目：一是当前用户下已存在同一 `(match_type='composite', normalized_match_value)` 的正式 `import_learning_rules`；二是同一 composite 下人工标注结果彼此冲突的样本组。`POST /api/bills/import/v2/learning/<session_id>/suggestions` 则额外接受当前导入预览选中的 `preview_updates[]`（以及可选 `previewIds[]`），先把这批本地编辑保存为 session annotation samples，再把 dry-run suggestions 按当前选择的 preview 来源做子集过滤，供导入预览里的 `Save as Long-term Learning` 建议弹窗只展示本次所选账单对应的 suggestion。响应结构固定为 `data{sessionId, totalCount, suggestions[]}`，其中每条 suggestion 至少包含 `matchType / matchValue / matchFeatures / sampleCount / sourcePreviewIds / learnedType / learnedCategoryId / learnedCategoryName / learnedSourceAccountId / learnedSourceAccountName / learnedDestinationAccountId / learnedDestinationAccountName / summary`。正式提升继续通过 `POST /api/bills/import/v2/learning/<session_id>/promote` 完成：当前请求体既支持沿用 `preview_updates[]` 先保存 annotation samples 再提升，也支持直接传入可选 `previewIds[]` 只提升当前 session 下已存在 annotation samples 的指定来源；若显式传入空 `previewIds[]`，接口会返回零提升而不会回退成整 session 全量提升。当前导入预览的运行时结构 map 为 `ImportTransactionCheckDataTab.vue -> ImportLearningSuggestionDialog.vue -> services.ts -> api/routes/bills.py -> BillService.get_import_learning_suggestions(...)/promote_session_annotations_to_learning(...) -> Database.save_import_annotation_samples(...)/promote_import_annotation_samples_to_learning(..., preview_ids=...)`。该切片当前仍不引入新的 suggestion 持久化表。
- 导入学习 suggestions / promote 两条会话级接口当前都会先校验 `import_sessions` 的当前用户可见性：缺失 session 或跨用户访问统一返回 `404 Import session not found`，避免把无效 session 误判成“零建议 / 零提升”的成功响应。
- 前端 `src/web/src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue` 当前也是用户触发“会话级学习助手”的主入口：用户在导入预览里选择 preview 行，调用 `getImportLearningSuggestions` / `promoteImportLearning` 来审核并提升长期学习规则；`src/web/src/views/desktop/learningcenter/ListPage.vue` 中的 LLM 页签则降级为兼容壳层，仅继续承载保存配置与历史 candidate 队列，不再作为推荐的主入口。
- `POST /api/llm/analyze-transactions` 当前也支持导入预览会话输入：请求体可携带 `session_id` 与 `preview_updates[]` / `preview_ids[]`，服务端会先同步当前 preview 草稿，再基于该 `import_session` 的 `bills_preview` 分组生成 `rule_induction` 型 `llm_candidates`。默认推荐入口是导入预览会话助手；Learning Center 的 LLM 页签仅保留配置与历史候选审查的兼容语义。
- matching 域当前已额外提供 `GET /api/matching/sessions/<session_id>/candidates` 作为导入会话级只读候选视图：接口按当前用户下的 `session_id` 读取整批 `preview[]`，再将每条 `preview.matching.transfer|investment|learning|recurring` 投影为显式 `candidates[]` 列表；响应结构固定包含 `session_id`、`summary(preview_count/candidate_count/counts_by_kind)` 与 `candidates[]`。其中每个 candidate 会携带稳定 `candidate_id=preview:<preview_id>:<kind>`、`kind`、`score/level/reason/status`、对应 `details`，以及从原预览复制的 `preview` 快照和 `context(dedup/parser/annotation)`，用于让前端或后续路由按 session 维度消费 matching 结果，而不必重新扫描整批 `preview[]`。
- matching 域当前还提供统一只读入口 `GET /api/matching/candidates`：请求必须二选一携带 `sessionId` 或 `billId`。当携带 `sessionId` 时，它复用导入会话级候选读模型并返回与 `/api/matching/sessions/<session_id>/candidates` 等价的 `data{session_id, summary, candidates[]}`；当携带 `billId` 时，它复用历史正式账单混合候选读模型并返回与 `/api/matching/bills/<bill_id>/candidates` 等价的 `data{billId, linkedPair, candidates[]}`，其中 `candidates[]` 当前可能同时包含 `transfer`、`investment` 与 `learning` family。
- 配对中心 investment settings 当前已拥有 dedicated matching-domain canonical API：`GET|PUT /api/matching/investment-settings` 负责读取/更新 `importLearningEnabled` 与 `investmentPlatformKeywords / investmentProductKeywords / investmentExcludeKeywords`，供 `src/web/src/views/desktop/pairingcenter/components/InvestmentRecognitionSettingsCard.vue` 作为正式写入链路使用。底层物理存储当前仍复用 `users.import_learning_enabled / investment_*_keywords` 字段，但 auth/profile 语义只保留兼容读写，不再是配对中心的官方入口。
- matching 域的 `billId` selector 当前已扩展为混合 historical candidate 读模型：除既有 `transfer` candidates 外，`GET /api/matching/bills/<bill_id>/candidates` 与 `GET /api/matching/candidates?billId=...` 还会返回 `kind="investment"` 的 formal-bill investment recommendation，以及 `kind="learning"` 的 formal-bill learning recommendation。investment 候选当前使用稳定 `candidateId=bill:<billId>:investment:<candidateBillId>`，并返回 `billId / score / level / reason / bill{...}`；它要求同用户、未参与其他 pair、金额绝对值相等且方向相反、来源账户不同、时间落在窗口内、正式账单文本仍命中 investment keyword 判定，且未命中 `bill_investment_pair_suppressions`。learning 候选继续使用带版本保护的稳定 `candidateId=bill:<billId>:learning:<ruleId>:<ruleRevision>`，并返回 `ruleId / score / level / reason / recommendedType / summary / suppressed`。
- matching 域当前还提供 `GET /api/matching/bills/<bill_id>/feedback` 作为 historical formal-bill 的 bill-scoped 审查读口：它会在当前用户下返回 `data{billId, events[]}`，其中每条 event 至少包含 `id / candidateId / action / createdAt / payload`，并按最新事件优先排序。该接口当前读取 `bill_pair_feedback` 中与目标 bill 相关的 append-only formal-bill feedback 事件，覆盖 `transfer / investment` family 的 generic accept|reject 结果；它不改变 pair list 读模型，也不扩到 preview family 或 formal-bill learning family。
- matching 域当前还提供 `POST /api/matching/candidates/<candidate_id>/accept` 作为 generic accept 的当前切片：它支持 `preview:<preview_id>:transfer`、`preview:<preview_id>:recurring`、`preview:<preview_id>:investment`、`preview:<preview_id>:learning`、`bill:<billId>:transfer:<candidateBillId>`、`bill:<billId>:investment:<candidateBillId>` 与 `bill:<billId>:learning:<ruleId>:<ruleRevision>`。`preview:<preview_id>:transfer` 分支继续复用导入预览写接口的 `accept` 语义，并要求请求体携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)` 以保持防陈旧写保护；成功后返回 `data{candidateId, action, previewId, sessionId, preview[]}`。`preview:<preview_id>:recurring` 分支则复用 preview recurring bind 写路径，要求请求体携带 `recurringId` 与 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)`；成功后返回 `data{candidateId, action, previewId, sessionId, recurringId, preview[]}`，并把当前 preview 重新绑定到目标 recurring candidate，使 session 级 recurring candidate 状态投影回 `confirmed`。之所以也要求 `reviewStatus`，是因为 recurring bind/clear 仍可能清理 transfer review feedback，所以 stale guard 需要把 transfer review 状态一起纳入快照。`preview:<preview_id>:investment` 分支则复用 preview-scoped investment feedback 写路径，要求请求体携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)`；成功后返回 `data{candidateId, action, previewId, sessionId, preview[]}`，并把 `bills_preview.preview_matching_feedback_json.investment` 更新为 `{review_status:"accepted", suppressed:false}`。该写路径不会覆盖当前 `preview_type / main_category / sub_category / recurring` 字段，但 follow-up 的 session 级 investment candidate 会继续保留并把状态投影为 `accepted`。`preview:<preview_id>:learning` 分支则复用 preview-scoped learning feedback 写路径，要求请求体携带 `ruleId` 与 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId, sourceAccountId, destinationAccountId)`；服务端只会接受当前 preview learning 读模型仍然暴露的同一条 rule。成功后返回 `data{candidateId, action, previewId, sessionId, preview[]}`，并把该 rule 已配置的 learned `type / category / source_account / destination_account` 结果应用到 `bills_preview.preview_type / preview_main_category / preview_sub_category / preview_source_account_id / preview_destination_account_id`，同时把 `bills_preview.preview_matching_feedback_json.learning` 更新为 `{review_status:"accepted", suppressed:false, rule_id, previous_preview, applied_preview}`。follow-up 的 session 级 learning candidate 会继续保留，并把状态投影为 `accepted`。`bill:<billId>:transfer:<candidateBillId>` 分支继续复用历史正式账单 `manual-pair` 写路径，在当前用户下建立 1:1 transfer/manual pair，并返回 `data{candidateId, action, pair}`；成功后还会向 `bill_pair_feedback` 追加一条 `action="accept"` 的 append-only 事件，记录 `candidate_id` 与最小 pair payload。`bill:<billId>:investment:<candidateBillId>` 分支则会写入 `bill_pair_links(pair_type='investment', source='manual')`，在当前用户下建立 1:1 investment/manual pair，并返回 `data{candidateId, action, pair}`；成功后同样会向 `bill_pair_feedback` 追加一条 `action="accept"` 的 append-only 事件。follow-up 的 `GET /api/matching/bills/<bill_id>/candidates` 与 `GET /api/matching/candidates?billId=...` 会返回 `linkedPair.pairType="investment"` 且 `candidates=[]`。`bill:<billId>:learning:<ruleId>:<ruleRevision>` 分支则会把 formal-bill learning candidate 视为一次有效 resolve：若 rule 对当前 bill 存在字段差异，就更新 `bills.type / main_category / sub_category / source_account_id / destination_account_id`；若 rule 已与当前 bill 对齐，则 accept 会保留 bill 当前字段不变。两种情况下它都会记录 `import_learning_rule_logs(action=accepted)`、增加该 rule 的 usage 计数，并把该 bill/rule 记为已解析的 suppression，从而避免 bill selector 后续重复暴露同一 candidate；成功后返回 `data{candidateId, action, bill}`。该接口当前不承载 reconcile-history 语义。
- matching 域当前还提供 `POST /api/matching/candidates/<candidate_id>/reject` 作为 generic reject 的当前切片：它现在支持 `preview:<preview_id>:transfer`、`preview:<preview_id>:investment`、`preview:<preview_id>:learning`、`preview:<preview_id>:recurring`、`bill:<billId>:transfer:<candidateBillId>`、`bill:<billId>:investment:<candidateBillId>` 与 `bill:<billId>:learning:<ruleId>:<ruleRevision>`。`preview:<preview_id>:transfer` 分支继续复用导入预览写接口的 `reject` 语义，并要求请求体携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)` 以保持防陈旧写保护；成功后返回 `data{candidateId, action, previewId, sessionId, preview[]}`。被拒绝的 transfer candidate 仍会保留在 session 级读模型中，但状态会投影为 `rejected`。`preview:<preview_id>:investment` 分支则面向导入预览里的 investment signal：它要求请求体携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)`，成功后返回 `data{candidateId, action, previewId, sessionId, preview[]}`，并把 preview-scoped feedback 写入 `bills_preview.preview_matching_feedback_json.investment{review_status:"rejected", suppressed:true}`；该写路径不会覆盖当前 `preview_type / main_category / sub_category / recurring` 字段，但 follow-up 的 session 级 investment candidate 会继续保留并把状态投影为 `rejected`。`preview:<preview_id>:learning` 分支则面向导入预览里的长期学习 recommendation：它同样要求请求体携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId, sourceAccountId, destinationAccountId)`，成功后返回 `data{candidateId, action, previewId, sessionId, preview[]}`，并把 preview-scoped feedback 写入 `bills_preview.preview_matching_feedback_json.learning{review_status:"rejected", suppressed:true}`。当该 learning candidate 仍处于 pending/rejected 轨道时，该写路径不会覆盖当前 `preview_type / main_category / sub_category / recurring` 字段；如果它此前已经通过 generic accept 应用过 learning rule，则 reject/clear 只会在当前 preview 仍匹配那次 accept 的 `applied_preview` 时恢复更早的 `previous_preview` snapshot；若用户在 accept 之后手工改过 learning 管辖字段，则 reject/clear 会保留这些较新的手工编辑，只更新 learning feedback 状态。`preview:<preview_id>:recurring` 分支则复用 preview recurring clear 写路径，要求请求体携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)`；成功后同样返回 `data{candidateId, action, previewId, sessionId, preview[]}`，并清除当前 preview 上的 recurring 绑定。该 recurring candidate 在 session 级读模型中会继续保留，但状态会从 `confirmed` 回到 `pending`；这是一种 preview family 级 reject/clear 语义，而不是按具体 `recurring_id` 的 suppressed 状态。`bill:<billId>:transfer:<candidateBillId>` 分支则面向历史正式账单 transfer-only family：成功后返回最小 `data{candidateId, action}`，并把该 logical pair 持久化写入 `bill_transfer_pair_suppressions`；同时会向 `bill_pair_feedback` 追加一条 `action="reject"` 的 append-only 事件，记录 `candidate_id` 与最小 logical-pair payload。后续 `GET /api/matching/bills/<bill_id>/candidates` 与 `GET /api/matching/candidates?billId=...` 会直接过滤掉这对账单，且同一 logical pair 之后也不能再通过 generic accept / manual pair 重新建立 pair。`bill:<billId>:investment:<candidateBillId>` 分支则会把该 logical pair 的 investment suppress 持久化写入 `bill_investment_pair_suppressions`；成功后同样会向 `bill_pair_feedback` 追加一条 `action="reject"` 的 append-only 事件。后续 bill selector 与统一 selector 会继续保留同一对账单的 transfer family，但过滤掉对应 investment family。`bill:<billId>:learning:<ruleId>:<ruleRevision>` 分支则会把该 bill/rule suppress 持久化写入 `bill_learning_rule_suppressions`，并让后续 bill selector 读取过滤对应 rule 级 learning candidate。其他未支持 family 统一返回 `400 Candidate family not supported`。
- matching 域当前还提供 `POST /api/matching/candidates/<candidate_id>/clear` 作为 generic clear 的当前切片：它目前支持 `preview:<preview_id>:learning` 与 `preview:<preview_id>:investment`。`preview:<preview_id>:learning` 分支要求请求体携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId, sourceAccountId, destinationAccountId)`，成功后返回 `data{candidateId, action, previewId, sessionId, preview[]}`。当 preview 仍与上一次 learning accept 的 `applied_preview` 一致时，clear 会恢复更早的 `previous_preview` snapshot；若用户在 accept 之后手工改过 learning 管辖字段，则 clear 会保留这些较新的手工编辑，只清除 `preview_matching_feedback_json.learning`，并让 session 级 learning candidate 状态回到 `pending`。`preview:<preview_id>:investment` 分支则要求请求体携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)`，成功后同样返回 `data{candidateId, action, previewId, sessionId, preview[]}`，并只清除 `bills_preview.preview_matching_feedback_json.investment`。当 live investment signal 仍存在时，follow-up 的 preview 行与 session 级 investment candidate 都会把状态重新投影回 `pending`；该写路径依旧不会改写 `preview_type / main_category / sub_category / recurring` 等 preview 业务字段。
- matching 域当前还提供 `POST /api/matching/reconcile-history` 作为历史正式账单 transfer-only 候选的批量读取入口：请求体必须携带非空 `billIds[]`，其中重复 id 会按首次出现顺序去重，且当前只支持显式账单列表，不支持范围扫描、investment / learning family 或任何写入语义。接口会逐条复用 `GET /api/matching/bills/<bill_id>/candidates` 的正式账单 transfer-only 读模型，并返回 `data{summary{billCount, candidateCount, linkedPairCount}, results[]}`；其中每个 `results[]` 项都沿用单账单候选读取的结构 `billId / linkedPair / candidates[]`。即便单账单 selector 已因 investment/manual pair 返回 `linkedPair`，该 batch 结果仍会强制保持 `linkedPair=null`，且不会把这类 investment link 计入 `linkedPairCount`。若任一 `billId` 不存在或不属于当前用户，则整个请求返回 `404 Bill not found`；非法请求体或非法 `billIds` 返回 `400`。它当前是一个 all-or-nothing 的 batch read orchestration，而不是新的 pairing / reject / reconcile write 语义。
 - matching 域当前还提供历史正式账单后配对 MVP：`GET /api/matching/bills/<bill_id>/candidates` 会按当前用户返回 `data{billId, linkedPair, candidates[]}`，其中 `linkedPair` 在账单已配对时包含 `id / pairType / source / leftBillId / rightBillId / otherBillId`，未配对时为 `null`；`candidates[]` 当前会混合投影 transfer / investment / learning 三类历史候选。transfer 候选继续要求同用户、未参与其他 pair、金额绝对值相等且方向相反、来源账户不同、时间落在窗口内、且未命中 `bill_transfer_pair_suppressions`，并为每项返回稳定 `candidateId=bill:<billId>:transfer:<candidateBillId>`、`billId / score / level / reason / bill{...}`。investment 候选当前沿用同一组金额/方向/账户/时间窗口约束，并额外要求正式账单文本仍命中 investment keyword 判定；对应结果使用稳定 `candidateId=bill:<billId>:investment:<candidateBillId>`，并在 `bill_investment_pair_suppressions` 命中时被过滤。learning 候选则继续走 `bill_learning_rule_suppressions` 的 resolved-suppression 读模型。对应写接口 `POST /api/matching/manual-pair` 当前接受请求体 `{ billId, candidateBillId, pairType? }`，其中 `pairType` 默认为 `transfer`，支持 `transfer | investment`；运行时结构 map 为 `POST /api/matching/manual-pair -> api/routes/matching.py::_parse_manual_pair_request() -> BillService.create_manual_transfer_pair() / create_manual_investment_pair() -> Database.create_manual_transfer_pair() / create_manual_investment_pair()`。`self-pair` 返回 `400`，账单不存在或不属于当前用户返回 `404`；当 `pairType=transfer` 时，重复/反向重复/任一账单已参与其他 transfer pair、已被 generic reject 持久化 suppress、或两条账单不满足 transfer-only 配对约束时返回 `409`；当 `pairType=investment` 时，冲突与候选约束会复用 investment/manual pair 规则并返回同样的 `409`。删除接口 `DELETE /api/matching/pairs/<id>` 当前会按 pair id 删除当前用户下的 manual pair：pair 不存在或不属于当前用户返回 `404`，非 manual source 返回 `409`；删除 transfer/manual pair 后会恢复对应 transfer candidates，删除 investment/manual pair 后则会恢复对应 investment candidates。列表接口 `GET /api/matching/pairs` 会按当前用户返回 `data{pairs[]}`，当前会读取 `bill_pair_links` 中已持久化的 manual pair 子集，并至少暴露 `transfer/manual` 与 `investment/manual` 两类 pair；每条 pair 仍附带 `id / pairType / source / leftBillId / rightBillId / createdAt / updatedAt` 以及 `leftBill / rightBill` 两侧正式账单的最小摘要，供后续 pairing center 或审查入口消费。该能力当前以 `bill_pair_links` + `bill_pair_feedback` + `bill_transfer_pair_suppressions` + `bill_investment_pair_suppressions` + `bill_learning_rule_suppressions` 作为最小持久化模型；其中 `bill_pair_feedback` 是 append-only 的 formal-bill feedback event stream，当前既承载 `transfer / investment` generic accept|reject 的 `candidate_id / action / payload_json` 写入，也通过 `GET /api/matching/bills/<bill_id>/feedback` 暴露 bill-scoped 审查视图，但仍不参与 pair 列表读模型。三类 suppressions 会在显式删除/批量删除账单、账户级删交易、去重、清空用户交易和清空用户业务数据时同步清理。
- 桌面端已入库交易详情当前已直接消费这条 historical matching 链路：运行时结构 map 为 `EditDialog.vue -> BillMatchingPanel.vue -> services.ts::getMatchingBillCandidates()/getMatchingBillFeedback()/acceptMatchingCandidate()/rejectMatchingCandidate()/deleteMatchingPair() -> GET /api/matching/candidates?billId=... / GET /api/matching/bills/<bill_id>/feedback / POST /api/matching/candidates/<candidate_id>/accept / POST /api/matching/candidates/<candidate_id>/reject / DELETE /api/matching/pairs/<id>`。该入口会在单条正式账单详情里展示 `linkedPair`、`transfer / investment / learning` 候选和 bill-scoped feedback 事件，并支持 generic accept / reject 以及清除当前 manual pair；当 formal-bill learning candidate accept 返回更新后的 `bill` 时，详情弹窗会同步刷新宿主交易字段；它当前是详情面板，不是新的独立 pairing center 页面。
- matching 域当前以 `bill_learning_rule_suppressions` 作为 formal-bill learning candidate 的 resolved-suppression 存储：`reject` 会写入 bill/rule suppress，`accept` 在成功应用 rule 后也会把该 bill/rule 标记为已解析，从而让后续 bill selector 读取不再重复暴露同一 learning candidate。该切片当前仍不引入 formal-bill learning pair link，而 `bill_pair_feedback` 也暂未扩到 learning family 或 preview family。
- 导入预览现已提供 `POST /api/bills/import/v2/preview-item/<preview_id>/transfer-decision` 作为转账建议决策接口：请求体除 `decision=accept|reject|clear` 外，还需要携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)` 作为防陈旧写保护；若预览状态在客户端读取后已变化，接口会返回 `409 Preview state changed, please refresh`，否则响应返回刷新后的整批 `preview[]`。`matching.transfer` 当前除候选分数外，还会携带 `review_status / reviewed_type / suppressed` 供前端展示用户决策状态，且 `clear` 会恢复 `accept` 前被覆盖的 `preview_type / category / recurring` 展示字段。当前 `reclassify` / `confirm` 提交的 `preview_updates[]` 若已存在与该决策相关的本地类型/分类/周期编辑，会额外携带 `clear_transfer_decision=true`，用于清除已持久化的 transfer 决策反馈，避免 accepted / rejected 状态与手工编辑结果分叉。
- 导入预览现已提供 `PUT /api/bills/import/v2/preview-item/<preview_id>/recurring-match` 与 `DELETE /api/bills/import/v2/preview-item/<preview_id>/recurring-match` 作为单条定时候选确认/清除接口：两者都要求 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)` 做防陈旧写保护，`PUT` 额外要求 `recurringId`。接口成功后返回刷新后的整批 `preview[]`；它只更新 `bills_preview.preview_recurring_*` 与相关 `matching.recurring` 展示字段，并会清除与手工周期决策冲突的 transfer review feedback，但不会在 preview 阶段提前推进 `recurring_bills.next_date`。当前导入弹窗 `Check Data` 的单条 `Choose Scheduled Match` / `Clear Scheduled Match` 已改为直接走这组接口。
- `GET /api/matching/sessions/<session_id>/candidates` 中的 `recurring` candidate 当前会根据 preview 绑定状态投影 `status`：`preview_recurring_id` 已写入时返回 `confirmed`，清除绑定但仍保留候选数量时返回 `pending`，从而让 session 级读模型与预览表当前状态保持一致。
- 导入预览 `Check Data` 的类型列当前会优先消费 `preview[].matching.*` 作为候选摘要来源：`transfer / learning / recurring` 继续映射为既有候选 chips，`investment` 已从只读 `Investment Signal` 升级为 preview-scoped `accept / reject / clear` 审查入口，`parser / dedup / annotation` 则作为补充上下文摘要显示；预览确认与重新分类请求结构不因这层展示消费而改变。`Investment Signal` 当前运行时结构 map 为 `ImportTransactionCheckDataTab.vue -> checkDataCandidateReview.ts::buildImportCheckDecisionExpectedState() -> services.acceptMatchingCandidate()/rejectMatchingCandidate()/clearMatchingCandidate() -> POST /api/matching/candidates/<candidate_id>/accept|reject|clear -> api/routes/matching.py -> BillService.apply_preview_investment_decision(...)`。其中 `candidate_id` 使用稳定格式 `preview:<preview_id>:investment`，请求体沿用 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)` stale-guard；成功响应仍返回 `data{candidateId, action, previewId, sessionId, preview[]}`，并且当前写路径只更新 `bills_preview.preview_matching_feedback_json.investment{review_status, suppressed}`，不会直接改写 `preview_type / main_category / sub_category / recurring` 等 preview 业务字段。
- `Annotation` 筛选中的 `Needs Review or Manually Annotated` 表示：交易仍存在待补分类/账户问题，或该交易已经被用户手工编辑过并视为人工标注。
- 当用户在导入预览表中行内编辑一条交易时，当前正在编辑的行会在标注筛选结果里保持可见，避免因分类/账户刚被补齐而立即从列表中消失。
- 当用户结束行内编辑时，该交易会被标记为人工标注；此后它仍命中 `Needs Review or Manually Annotated`，但不会再命中 `No Annotation Issues`。

### 5.3 统计与汇率
- 统计主链当前统一由 `GET /api/statistics/*` 提供
- 当前前端统计主链已切到：
  - `GET /api/statistics/category-statistics`
  - `GET /api/statistics/category-statistics/trends`
  - `GET /api/statistics/asset-trends`
- 旧 `transaction-statistics*` 兼容子路径已删除，当前通过 legacy 404 回归测试防止恢复
- 汇率主链已切到 `GET /api/statistics/exchange-rates`
- 用户自定义汇率写接口已收口到统计域 REST：`PUT /api/statistics/exchange-rates/custom`、`DELETE /api/statistics/exchange-rates/custom/<currency>`；旧 `v1/exchange_rates/user_custom/update.json` 与 `v1/exchange_rates/user_custom/delete.json` 已停止使用，并由 legacy 404 回归保护
- `GET /api/statistics/exchange-rates` 在用户存在自定义汇率时优先返回 `dataSource=user_custom` 的持久化结果，否则继续返回实时/回退汇率
- 用户资料页“数据统计”已切到认证域 REST 主链 `GET /api/data/statistics`，旧 `v1/data/statistics.json` 已删除并由 legacy 404 回归保护
- 用户数据管理已继续收口到认证域 REST：`GET /api/data/export.csv`、`GET /api/data/export.tsv`、`POST /api/data/clear/transactions`、`POST /api/data/clear/all`；旧 `v1/data/export.csv`、`v1/data/export.tsv`、`v1/data/clear/transactions.json`、`v1/data/clear/all.json` 已停止使用，并由 legacy 404 回归保护
- 系统版本检查已切到 `GET /api/system/version`；旧 `v1/systems/version.json` 已停止使用，并由 legacy 404 回归保护
- 导入解析辅助已切到 `POST /api/bills/parse_import`；前端 `services.ts` 不再使用旧 `v1/transactions/parse_import.json`
- 账单写入/导入辅助旧 rewrite 已移除：`v1/transactions/add.json`、`v1/transactions/modify.json`、`v1/transactions/delete.json`、`v1/transactions/import.json`、`v1/transactions/reconciliation_statements.json` 当前应返回 404
- 分类写入旧 rewrite 已移除：`v1/transaction/categories/list.json`、`v1/transaction/categories/add.json`、`v1/transaction/categories/add_batch.json` 当前应返回 404
- 导入提交前端已统一走 `POST /api/bills/import/v2/confirm`；旧 `v1/transactions/import/process.json` 与 `v1/transactions/parse_dsv_file.json` 已停止使用，并由 legacy 404 回归保护
- AI 小票识图前端已切到 `POST /api/ml/receipt-recognition`；当前 Python 后端提供 disabled-safe `501 Not Implemented` 占位语义，旧 `v1/llm/transactions/recognize_receipt_image.json` 已停止使用，并由 legacy 404 回归保护
- 交易列表前端主链已切到 `GET /api/bills/`；按月列表主链已切到 `GET /api/bills/by-month`
- 旧 `v1/transactions/list.json` 与 `v1/transactions/list/by_month.json` 已移除，并由 legacy 404 回归保护
- 交易图片前端主链已切到 `POST /api/bills/pictures` 与 `POST /api/bills/pictures/unused`
- 旧 `v1/transaction/pictures/upload.json` 与 `v1/transaction/pictures/remove_unused.json` 已停止使用，并由 legacy 404 回归保护
- 当前运行态前端源码（`src/web/src`）已不再直接引用 `v1/` 或 `/api/v1/` 路径；legacy 兼容仅保留在后端兼容层与历史 `ezbookkeeping/` 快照中
- `src/bill_analyser/api/app.py` 中历史 `URLRewriteMiddleware` 已移除；当前运行态不再通过 WSGI rewrite 兼容任何 `/api/v1/*` 路径
- 认证入口主链已统一为 `POST /api/auth/login`、`POST /api/auth/register`、`POST /api/auth/logout`；旧 `authorize.json`、`register.json`、`logout.json` 已停止使用，并由 legacy 404 回归保护
- 认证辅助入口已继续收口到 REST：`POST /api/auth/email/verify`、`POST /api/auth/email/resend-verification`、`POST /api/auth/password/forgot`、`POST /api/auth/password/reset`；旧 `verify_email/*.json` 与 `forget_password/*.json` 已停止使用，并由 legacy 404 回归保护
- 用户资料主链已扩展到头像 REST：`GET|PUT /api/profile`、`POST|DELETE /api/profile/avatar`；旧 `v1/users/avatar/update.json` 与 `v1/users/avatar/remove.json` 已停止使用，并由 legacy 404 回归保护
- 用户资料主链已扩展到验证邮件重发 REST：`POST /api/profile/email/resend-verification`；旧 `v1/users/verify_email/resend.json` 已停止使用，并由 legacy 404 回归保护
- 用户资料主链已扩展到第三方登录 REST：`GET /api/profile/external-auths`、`POST /api/profile/external-auths/unlink`；旧 `v1/users/external_auth/*.json` 已停止使用，并由 legacy 404 回归保护
- 用户资料主链已扩展到应用云同步设置 REST：`GET|PUT|DELETE /api/profile/cloud-settings`；旧 `v1/users/settings/cloud/*.json` 已停止使用，并由 legacy 404 回归保护
- 2FA 主链已扩展到完整 REST 写接口：`GET /api/2fa/status`、`POST /api/2fa/enable/request`、`POST /api/2fa/enable/confirm`、`POST /api/2fa/disable`、`POST /api/2fa/recovery/regenerate`；旧 `v1/users/2fa/*.json` 写接口已停止使用，并由 legacy 404 回归保护
- 2FA 登录验证链路已补齐 REST：`POST /api/2fa/verify`、`POST /api/2fa/recovery/verify`；`/api/auth/login` 在用户启用 2FA 时返回 `need2FA` 与待验证 token，旧 `/api/2fa/authorize.json`、`/api/2fa/recovery.json` 已停止使用，并由 legacy 404 回归保护
- 2FA 恢复码当前已从纯内存缓存升级为数据库持久化哈希存储：`db.py` 新增 `user_two_factor_recovery_codes` 表，启用 2FA / 重生成恢复码时会整体替换当前批次，恢复码登录校验按哈希一次性消费，旧批次在重生成后立即失效
- 安全中心当前已补齐 step-up 验证基础：`POST /api/security/step-up/verify` 可通过当前密码或 TOTP passcode 签发短期 `step_up` token，`POST /api/2fa/disable`、`POST /api/2fa/recovery/regenerate`、`POST /api/data/clear/transactions`、`POST /api/data/clear/all` 已支持使用 `stepUpToken` 替代重复提交密码
- 备份主链当前除创建 / 下载 / 删除 / 恢复外，还支持恢复前预验证与可选本地加密：`GET /api/backup/` 与 `POST /api/backup/create` 会返回备份 `checksum`、压缩包结构检查结果与 `ready_to_restore` 摘要；当存在 `BILL_ANALYSER_BACKUP_ENCRYPTION_KEY` 时，创建接口会产出 `.zip.enc` 本地加密备份并在记录中标记 `encrypted=true`；`POST /api/backup/restore/verify` 与 `POST /api/backup/restore/<filename>` 会自动识别并解密本地加密备份；`POST /api/backup/cleanup` 现已同时覆盖 `.zip` / `.zip.enc`，并优先基于 `backup_records` 决定 retention 淘汰项与回写 `deleted` 状态
- 认证域敏感安全动作当前会同时写认证日志与操作审计：`2fa_enabled`、`2fa_disabled`、`2fa_recovery_regenerated`、`2fa_recovery_code_used` 通过 `audit_logs` 持久化关键安全事件上下文
- 备份域关键动作当前也会写入 `audit_logs`：`backup_created`、`backup_deleted`、`backup_restored`、`backup_restore_verified` 会记录备份文件名、校验值、预验证结果或失败原因，便于后续追踪恢复链路与异常排查
- OAuth2 callback authorize 已收口到 `POST /api/auth/oauth2/authorize`；旧 `/api/oauth2/authorize.json` 已停止使用，并由 legacy 404 回归保护。当前后端仅提供 disabled-safe / not-implemented 语义，待后续真实 OAuth2 provider exchange 主链补齐
- token 会话主链已切到认证域 REST：`GET|DELETE /api/tokens`、`POST /api/tokens/api`、`POST /api/tokens/mcp`、`POST /api/tokens/refresh`、`DELETE /api/tokens/<id>`；旧 `v1/tokens/generate*.json`、`v1/tokens/revoke*.json` 与 `v1/tokens/refresh.json` 已收口到新主链，并由 legacy 回归保护
- 认证错误契约已补齐：`POST /api/auth/login` 与 `POST /api/tokens/refresh` 在请求体为空、`null` 或其他非对象 JSON 时返回 `400 Invalid request`，不再落入 `500`。
- 统计时间范围契约已收紧：`GET /api/statistics/category-statistics`、`GET /api/statistics/category-statistics/trends`、`GET /api/statistics/asset-trends` 在起始时间/年月晚于结束时间时返回 `400`，避免把反向区间静默视为空结果。

---

## 6. 数据库与数据流

### 6.1 主要业务表（由 `db.py` façade 通过 `DatabaseSchemaMixin` 初始化）
- 交易域：`bills`
- 匹配域：`bill_pair_links`、`bill_pair_feedback`
- 分类域：`categories`
- 账户域：`accounts`、`account_types`、`account_transfers`
- 标签域：`tags`、`bill_tags`
- 模板域：`bill_templates`、`recurring_bills`
- 预算域：`budgets`、`budget_history`
- 用户与安全：`users`、`sessions`、`auth_logs`、`audit_logs`、`user_two_factor_recovery_codes`
- 备份与恢复：`backup_records`、`backup_jobs`
- 导入三阶段：`import_sessions`、`bills_parser_template`、`bills_preview`
- 导入三阶段临时表当前还会持久化解析器元标签：`bills_parser_template.parser_tags_json` 保存解析阶段 tags，`bills_preview.preview_parser_tags_json` 保存预览阶段 tags
- 迁移与索引补齐由 schema 子模块统一编排；运行态调用方不直接依赖某个单独 schema 文件

### 6.1.1 模板域当前漂移清单
- 前端期望字段：`templateType/categoryId/sourceAccountId/destinationAccountId/sourceAmount/destinationAmount/hideAmount/tagIds/displayOrder/hidden/scheduled*`。
- 后端当前字段：`category/account/tag/description/is_favorite/use_count/last_used_at` 等旧模板语义。
- 结果：已通过 DTO → 数据库映射完成首轮收口，但模板双表语义仍比账户/标签复杂，兼容层清理应继续分阶段推进。

### 6.2 导入处理顺序（核心）
1. 解析器识别并标准化账单
2. 数据验证
3. 智能去重
4. 分类匹配（规则+类型过滤）
5. 账户匹配（源/目标账户）
6. 预览写入或正式入库

---

## 7. 日志与运维

### 7.1 日志系统
- 统一日志入口：`src/bill_analyser/utils/logger.py`
- 支持异步写入、滚动清理、等级分离
- 常用日志目录：`logs/`
- 已删除 `src/bill_analyser/utils/advanced_logger.py` 等已确认退出运行态的平行实现，并由回归测试持续阻止回流。

### 7.2 启停脚本
- 一键启动：`一键启动.bat` / `一键启动.ps1`
- 分别启动：`start_backend.ps1`、`start_frontend.ps1`
- 停止服务：`停止服务器.ps1`

---

## 8. 测试结构与质量门禁

### 8.1 测试分布（`tests/`）
- 核心测试：`test_import.py`、`test_smart_dedup.py`、`test_category_engine_v2.py`、`test_db.py`
- API 测试：`test_v1_routes.py`、`test_statistics_*`、`new_ui/` 下接口测试
- 领域化测试：`tests/domains/` 采用 `domain/(unit|integration)` 结构，当前已覆盖认证、统计、导入主链、数据库深水区等高价值路径
- 回归脚本/诊断脚本：`check_*`、`diagnose_*`、`debug_*`
- 前端 Jest 测试：`tests/web/`（与 Python 测试同仓库级根目录并行管理）

### 8.2 推荐验证命令
- 单元/集成：
  - `C:/Users/hcy/OneDrive/Github/bill_analyser/.venv/Scripts/python.exe -m pytest tests/ -v`
- 代码质量：
  - `C:/Users/hcy/OneDrive/Github/bill_analyser/.venv/Scripts/python.exe -m pylint src/bill_analyser/core/*.py src/bill_analyser/api/routes/*.py`

---

