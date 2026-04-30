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
- `matching.py`：导入会话级 matching candidate 投影、preview family generic accept/reject、历史正式账单后配对与 pair 列表接口，以及规则中心 investment pairing settings 的 canonical REST 读写接口
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
- `db_budgets_core.py` / `db_budgets_execution.py` / `db_budgets_forecast.py`：预算主数据、分类上下文与分组 helper，以及执行统计 / 历史 / 预测 / 导入导出查询；预算分类上下文会把历史 `categories.type=1` 归一为当前支出类型 `3`，避免旧分类预算在执行统计和预算列表中被类型过滤漏掉
- `db_budgets_reporting.py`：预算 reporting 兼容聚合层；运行时通过它组合 execution/history 与 forecast/import/export 两个预算域 mixin
- `db_import_configs.py` / `db_import_sessions.py` / `db_import_preview.py` / `db_import_learning.py` / `import_learning/`：导入模板配置、三阶段会话、预览编辑/确认、长期学习 durable corpus、exact/manual 学习规则、session-scoped dry-run suggestions，以及基于 dataset snapshot / active model registry 的轻量双头学习模型训练与推理
- `bill_service.py`：导入主流程编排（含 v2 三阶段导入）
- `smart_dedup.py`：智能去重引擎主实现；导入批次内按“完全重复 → 平台-银行去重 → 转账配对 → 相似去重 → 分账去重”的顺序处理，含数据库对比时再追加数据库重复检测与跨批次转账配对。平台-银行去重优先保留支付平台账单；同号平台/银行候选直接按重复处理，异号候选只有在没有明确转账意图且具备文本重复证据时才先于转账配对吸收，避免把真实转账错误归并为平台-银行重复。
- `category_engine.py`：分类规则匹配式解析与分类匹配（含类型过滤与预编译优化）；运行时仅从 `category_rules` canonical source 加载规则，不再回退到 `categories.keywords`。`rule_expression` 后端兼容旧 `OR:a|b&AND:c&NOT:d`，正式语法由 `OR={...}`、`AND={...}`、`NOT={...}`、`REGEX={...}` 子句组成：`+` 表示 AND，`/` 表示同一表达式内 OR，`|` 表示表达式级 OR，`×` 或 `NOT` 表示 AND NOT；多层括号按 AST 优先级执行。前端共享规则构建器把一条 canonical 分类规则呈现为“多个表达式 + 表达式内多个规则块”，支持多层括号、连接符与 OR/AND/NOT 标签分离展示，并在同一表达式内保持 `/` 连接块同组展示；迁移旧关键词会转义分隔符与括号，避免特殊字符关键词被拆成错误语义。分类规则运行时匹配顺序由 `categories.priority`、`category_id` 与规则 id 的稳定顺序决定，`category_rules.priority` 仅作为兼容元数据保留；加载规则时会把历史 `categories.type=1` 归一为支出类型，匹配结果会保留 canonical category id/type，无法解析到真实分类时 fail closed，不生成同名虚拟分类。
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
- `desktop/pairingcenter/ListPage.vue` 当前承载桌面端 canonical 规则中心：一级导航分为“配对总览”“规则配置”“长期学习”“LLM 识别”。“配对总览”下有“转账配对 / 投资配对”，读取并管理历史正式账单 pair；“规则配置”下有“分类识别 / 周期识别”，分别承接 category rules 与 recurring matching 入口，投资类识别规则也作为投资分类下的 category rule 表达式在“分类识别”中维护；“长期学习”下保留自动建议与学习规则；“LLM 识别”下保留 LLM 规则候选与 LLM 配置。页面以预算管理式纵向导航 + 二级 tabs 展示当前域，右侧标题栏统一承载当前二级标题、刷新/新增/生成建议/筛选等动作，正文子组件不再重复渲染同名标题；分类识别表格的批量选择、表头筛选、每页数量与分页统计都收敛在表头/分页区，分类表头筛选按收入 / 支出 / 转账 / 投资聚类并在视口内滚动，避免选项过多时撑高页面。
- `/pairing/list` 是规则中心运行态主路由，并使用 `domain/tab` query 表达当前业务域与二级视图；UI 的一级导航由 `domain/tab` 派生：`domain=transfer|investment&tab=overview` 属于“配对总览”，`domain=transfer&tab=rules` 属于“规则配置”，`domain=learning` 属于长期学习，`domain=llm` 属于 LLM 识别。旧深链 `pairType` / `view` / `tab` query 会被归一化到新 domain/tab；旧投资识别设置深链会改写到 `domain=transfer&tab=rules` 的分类识别页；`/rules/center` 与 `/learning/center` 只重定向到同一规则中心页面，不再保有独立持久化业务中心语义。
- `desktop/budgets/ListPage.vue` 的历史预算执行视图使用 `historyPolarChart.ts` 生成往期预算执行图：分类隐藏/显示通过 legend selection 同步到柱状预算金额、圆环分组与标签，剩余可见分类会重新填满极坐标布局；一级/二级分类标签保留上一帧角度状态，跨 0° 时按最短圆弧平滑过渡。图表构建会在空历史、无可见分类或非法几何时返回安全空态/兜底布局，避免往期预算执行图不可见。
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
- 导入弹窗 `Check Data` 步骤的右上角筛选入口使用分组下拉菜单，当前顺序与表头/行首状态保持一致：先给出 `人工标注`、`信号`，再给出 `时间范围`、`类型`、`分类`、`账户`、`标签`、`备注/描述`。其中 `时间范围` 预设包含 `全部`、`本周`、`本月`、`本年` 与 `自定义`。在 server-paged 模式下，前端会先加载整会话轻量索引，再据此做全局筛选、待标注计数与排序切页，不再局限于“当前已访问页”。
- `/api/bills/parse_import` 返回的导入预览项当前会同时携带 `parserSource` 与 `parserTags`；`BillService.get_import_preview()` / `/api/bills/import/v2/dedup` 预览数据会返回 `preview_parser_id`、`preview_parser_tags` 与解析到的 `category_id`，而 `/api/bills/import/v2/preview/<session_id>` 的简化结果也会附带 `parserSource` 与 `parserTags` 供前端保留与消费解析器元信息。该 server-paged 预览族当前包含两条读模型：一是 `/api/bills/import/v2/preview/<session_id>`，支持 `sort_by` + `sort_direction` 的真实后端排序契约，并额外接受 `preview_ids` 以按前端全局筛选/排序结果回填当前页详情；受支持列为 `time / type / sourceAmount / counterparty / paymentMethod / comment`；二是 `/api/bills/import/v2/preview/<session_id>/index`，返回整会话轻量索引，供前端在不拉全量详情行的前提下做全局筛选、计数与排序分页。
- `BillService.get_import_preview()` / `/api/bills/import/v2/dedup` 当前会为每条预览账单附带嵌套 `matching` 结构；运行态 preview family 仍包含 `transfer / investment / learning / llm / recurring / dedup / parser / annotation`。其中 `investment` 不再来自分类后的第二套关键词 detector，而是直接根据当前 preview 已落定的 canonical `preview_type="投资"` / 分类结果投影为 matching 状态；黄色 `matching.llm` 则由 `preview_matching_feedback_json.llm` 回填最近一次 LLM 推荐/接受/拒绝状态与建议分类/账户链路，只作用于 `bills_preview` draft，不写正式 `bills`；旧平铺 `investment_signal_*` 与 `investment_platform/product` 字段仅保留兼容空值，不再承载运行时识别信号。
- 分类规则的 create/update/delete/reorder/migrate 会刷新规则中心使用的全局 `CategoryEngine`，并同步刷新 `BillService` 导入预览所用分类引擎，确保导入预览分类匹配读取当前 `category_rules` canonical 规则而不是旧实例缓存。
- 导入预览来自 `SmartDeduplicationEngine` 的 `dedup_type` 与 `dedup_source_ids`：批次内平台-银行去重在转账配对之前执行，只有明确转账意图或缺少平台-银行重复文本证据的异号候选才会进入转账配对；`dedup_type=transfer` 的预览信号 tooltip 会从当前预览行的 `parserSource/parserTags` 与 `dedup_source_ids` 指向的来源行收集解析器标签，展示成类似“匹配 | 民生银行 | 支付宝”的来源链，而不是只显示去重类型名称。
- 导入学习域当前分为 durable corpus、exact/manual overlay、session dry-run 和 active model 四层：`Database.save_import_annotation_samples(...)` 会继续写 session 临时表 `import_annotation_samples`，同时把同一 preview 的 parser-aware 特征与人工标注写入 `import_learning_corpus_samples`；语料变化后会生成 `import_learning_dataset_snapshots`，训练 `import_learning-dual-head` 轻量模型，并把 feature schema / model / policy 版本和模型参数引用写入 `import_learning_model_registry` 的 active 记录。模型采用共享特征编码和 semantic / route 双头输出，分别预测 `type/category` 与 `source_account/destination_account`，样本不足时只保留 insufficient snapshot，不输出伪模型信号。`import_learning_rules` 仍是 exact/manual overlay 真值层，优先级高于模型；导入预览命中 exact/manual composite 时不会暴露模型推荐，`import_learning_enabled=false` 时也不会读取 active model。preview learning 信号分为绿色与蓝色：绿色只展示模型建议，accept / reject+correction 会写入 corpus 与 feedback 并刷新 active model；蓝色必须满足超过 2 次 accepted confirmations、confidence/margin 与冲突检查后才会自动应用到当前 `bills_preview` draft，撤销会恢复 preview draft、写 rollback feedback，并抑制同一 preview 后续刷新再次自动应用。`GET /api/bills/import/v2/learning/<session_id>/suggestions` 仍读取当前 session 的 `import_annotation_samples` 与 `bills_preview` 生成 dry-run suggestions；`POST` 入口可先保存 `preview_updates[]` 并按 `previewIds[]` 裁剪本次选择。正式提升继续写入 `import_learning_rules`，Learning Center 的跨 session 挖掘继续使用 `import_learning_corpus_samples` 生成 `import_learning_suggestions`，accept / reject / preview apply / rollback / auto apply 会写 `import_learning_feedback_events` 与 `import_learning_concept_stats` 作为可观测事件和计数底座。
- 导入学习 suggestions / promote 两条会话级接口当前都会先校验 `import_sessions` 的当前用户可见性：缺失 session 或跨用户访问统一返回 `404 Import session not found`，避免把无效 session 误判成“零建议 / 零提升”的成功响应。
- 前端 `src/web/src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue` 是导入会话内触发长期学习建议与 LLM 规则归纳的主入口：用户在导入预览里选择 preview 行，调用 `getImportLearningSuggestions` / `promoteImportLearning` 来审核并提升长期学习规则；相关持久化学习与 LLM 配置治理由规则中心 `/pairing/list?domain=learning|llm&tab=...` 承接。
- `POST /api/llm/analyze-transactions` 当前也支持导入预览会话输入：请求体可携带 `session_id` 与 `preview_updates[]` / `preview_ids[]`，服务端会先同步当前 preview 草稿，再基于该 `import_session` 的 `bills_preview` 分组生成 `rule_induction` 型 `llm_candidates`。导入会话错误按稳定 `code/error_code` 分流：LLM 未启用返回 `400 LLM_DISABLED`，会话不存在返回 `404 IMPORT_SESSION_NOT_FOUND`，未选择有效 preview 返回 `400 PREVIEW_SELECTION_EMPTY`，已选行缺少可归纳的分类或文本上下文返回 `422 PREVIEW_SELECTION_INSUFFICIENT`，provider 运行失败返回 `503 LLM_PROVIDER_UNAVAILABLE`，限流返回 `429 LLM_RATE_LIMITED`。
- 黄色 LLM 预览推荐当前走 `POST /api/llm/preview-recommend`、`POST /api/llm/preview-recommend/accept`、`POST /api/llm/preview-recommend/reject` 与 `GET /api/llm/memory` 四条 REST 入口：服务端会先按 `preview_updates[]` 同步当前 draft，再基于最近 `llm_memory_events(feedback)`、现有分类路径与账户名构造 prompt，为选中的 preview 行生成分类/账户建议。recommendation 只会补空白的 `preview_main/sub_category` 与 `preview_source/destination_account_id`，不会覆盖 exact/manual/learning 已落定的 draft 字段；accept 保留已补齐的 preview draft，reject 在当前 preview 仍匹配已应用快照时回滚到推荐前快照。recommendation/feedback 都会追加写入 append-only `llm_memory_events`，而 `preview_matching_feedback_json.llm` 只保存该 preview 的当前黄色信号状态，供导入预览 `Signals` 列展示 `pending/accepted/rejected` 与建议详情。
- LLM provider factory 当前支持 `openai`、`claude`/legacy `anthropic`、`ollama`，并以 OpenAI-compatible 路径支持 `deepseek`、`xai`、`google`、`openrouter`、`openai_compatible`/`openai-compatible`、`azure`/`azure_openai` 等别名；规则中心 LLM config 表单只把 provider 选择作为配置值保存，模型/Base URL 仍由用户显式填写，API key 字段不会从已保存配置回填到前端 state。已保存的 `llm_configs` 还通过 `advanced_settings` JSON 保存高级参数（reasoning depth、temperature、max tokens、system prompt、classification/rule prompt template），旧库初始化时会自动补列；运行时按当前请求的 `user_id` 解析该用户自己的 active config，并把高级参数注入 `LLMLearningService` 与 provider 调用，不会把某个用户的 API key/Base URL/提示词写入进程全局 `LLM_CONFIG`；列表/创建/更新/激活响应继续只返回脱敏后的配置。
- matching 域当前已额外提供 `GET /api/matching/sessions/<session_id>/candidates` 作为导入会话级只读候选视图：接口按当前用户下的 `session_id` 读取整批 `preview[]`，再将当前运行态活跃的 `preview.matching.transfer|learning|recurring` 投影为显式 `candidates[]` 列表；响应结构固定包含 `session_id`、`summary(preview_count/candidate_count/counts_by_kind)` 与 `candidates[]`。其中每个 candidate 会携带稳定 `candidate_id=preview:<preview_id>:<kind>`、`kind`、`score/level/reason/status`、对应 `details`，以及从原预览复制的 `preview` 快照和 `context(dedup/parser/annotation)`，用于让前端或后续路由按 session 维度消费 matching 结果，而不必重新扫描整批 `preview[]`。导入预览中的投资识别已收口到 canonical 分类链路与最终 `preview_type/category` 结果，不再额外暴露 preview-scoped `investment` candidate family。
- matching 域当前还提供统一只读入口 `GET /api/matching/candidates`：请求必须二选一携带 `sessionId` 或 `billId`。当携带 `sessionId` 时，它复用导入会话级候选读模型并返回与 `/api/matching/sessions/<session_id>/candidates` 等价的 `data{session_id, summary, candidates[]}`；当携带 `billId` 时，它复用历史正式账单混合候选读模型并返回与 `/api/matching/bills/<bill_id>/candidates` 等价的 `data{billId, linkedPair, candidates[]}`，其中 `sessionId` 读模型当前包含 `transfer`、`learning` 与 `recurring` family，而 `billId` 读模型当前可能同时包含 `transfer`、`investment` 与 `learning` family。
- 分类规则迁移入口 `POST /api/category-rules/migrate` 当前同时承接两类持久化规则导入：旧 `categories.keywords` 会转换为 canonical `category_rules.rule_expression`；用户行中的投资平台 / 产品 / 排除关键词会转换为一条投资分类规则表达式（例如 `OR={平台...}+OR={产品...}+NOT={排除...}`），落到当前用户最匹配的投资类型分类下。规则中心不再提供独立的“投资识别设置”页面；投资类交易识别的可编辑主链统一在分类规则系统中维护，旧 `GET|PUT /api/matching/investment-settings` 只保留确定性的 `410 Gone` 退役响应，避免隐藏写入口继续修改旧关键词配置。
- matching 域的 `billId` selector 当前已扩展为混合 historical candidate 读模型：除既有 `transfer` candidates 外，`GET /api/matching/bills/<bill_id>/candidates` 与 `GET /api/matching/candidates?billId=...` 还会返回 `kind="investment"` 的 formal-bill investment recommendation，以及 `kind="learning"` 的 formal-bill learning recommendation。investment 候选当前使用稳定 `candidateId=bill:<billId>:investment:<candidateBillId>`，并返回 `billId / score / level / reason / bill{...}`；它要求同用户、未参与其他 pair、金额绝对值相等且方向相反、来源账户不同、时间落在窗口内、正式账单文本仍命中 investment keyword 判定，且未命中 `bill_investment_pair_suppressions`。learning 候选继续使用带版本保护的稳定 `candidateId=bill:<billId>:learning:<ruleId>:<ruleRevision>`，并返回 `ruleId / score / level / reason / recommendedType / summary / suppressed`。
- matching 域当前还提供 `GET /api/matching/bills/<bill_id>/feedback` 作为 historical formal-bill 的 bill-scoped 审查读口：它会在当前用户下返回 `data{billId, events[]}`，其中每条 event 至少包含 `id / candidateId / action / createdAt / payload`，并按最新事件优先排序。该接口当前读取 `bill_pair_feedback` 中与目标 bill 相关的 append-only formal-bill feedback 事件，覆盖 `transfer / investment` family 的 generic accept|reject 结果；它不改变 pair list 读模型，也不扩到 preview family 或 formal-bill learning family。
- matching 域当前还提供 `POST /api/matching/candidates/<candidate_id>/accept|reject|clear` 作为 preview family / historical family 的 generic 决策入口。导入预览运行态活跃的 preview family 当前包含 `transfer / learning / recurring`：`preview:<preview_id>:transfer` 继续复用导入预览写接口并要求 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)` 做防陈旧写保护；`preview:<preview_id>:learning` 继续复用 preview-scoped learning feedback 写路径，并在 accept 时把 learned `type / category / source_account / destination_account` 应用到当前 preview；模型来源的 learning candidate 还会校验客户端提交的 `modelVersion` 与当前 live recommendation 一致，且模型引用的分类/账户必须仍存在，否则返回冲突/不可用而不写入 preview；`preview:<preview_id>:recurring` 继续复用 preview recurring bind/clear 写路径，把 session 级 recurring candidate 状态在 `confirmed/pending` 间投影。preview family 的 generic accept/reject/clear 请求体当前还支持可选 `responseMode="preview-item"`：命中该模式时，响应会优先返回 `data.previewItem` 单条刷新后的 preview 投影，而不是强制携带整批 `preview[]`；historical bill family 继续沿用原有 bill/pair 结果结构。导入预览不再暴露独立的 `preview:<preview_id>:investment` generic candidate family；投资识别结果直接体现在 canonical 分类链路命中的 `preview_type/category` 上。历史正式账单 `bill:<billId>:investment:<candidateBillId>` family 与对应 manual pair / suppression / feedback event stream 保持不变。
- matching 域当前还提供 `POST /api/matching/reconcile-history` 作为历史正式账单 transfer-only 候选的批量读取入口：请求体必须携带非空 `billIds[]`，其中重复 id 会按首次出现顺序去重，且当前只支持显式账单列表，不支持范围扫描、investment / learning family 或任何写入语义。接口会逐条复用 `GET /api/matching/bills/<bill_id>/candidates` 的正式账单 transfer-only 读模型，并返回 `data{summary{billCount, candidateCount, linkedPairCount}, results[]}`；其中每个 `results[]` 项都沿用单账单候选读取的结构 `billId / linkedPair / candidates[]`。即便单账单 selector 已因 investment/manual pair 返回 `linkedPair`，该 batch 结果仍会强制保持 `linkedPair=null`，且不会把这类 investment link 计入 `linkedPairCount`。若任一 `billId` 不存在或不属于当前用户，则整个请求返回 `404 Bill not found`；非法请求体或非法 `billIds` 返回 `400`。它当前是一个 all-or-nothing 的 batch read orchestration，而不是新的 pairing / reject / reconcile write 语义。
- matching 域当前还提供历史正式账单后配对 MVP：`GET /api/matching/bills/<bill_id>/candidates` 会按当前用户返回 `data{billId, linkedPair, candidates[]}`，其中 `linkedPair` 在账单已配对时包含 `id / pairType / source / leftBillId / rightBillId / otherBillId`，未配对时为 `null`；`candidates[]` 当前会混合投影 transfer / investment / learning 三类历史候选。transfer 候选继续要求同用户、未参与其他 pair、金额绝对值相等且方向相反、来源账户不同、时间落在窗口内、且未命中 `bill_transfer_pair_suppressions`，并为每项返回稳定 `candidateId=bill:<billId>:transfer:<candidateBillId>`、`billId / score / level / reason / bill{...}`。investment 候选当前沿用同一组金额/方向/账户/时间窗口约束，并额外要求正式账单文本仍命中 investment keyword 判定；对应结果使用稳定 `candidateId=bill:<billId>:investment:<candidateBillId>`，并在 `bill_investment_pair_suppressions` 命中时被过滤。learning 候选则继续走 `bill_learning_rule_suppressions` 的 resolved-suppression 读模型。对应写接口 `POST /api/matching/manual-pair` 当前接受请求体 `{ billId, candidateBillId, pairType? }`，其中 `pairType` 默认为 `transfer`，支持 `transfer | investment`；运行时结构 map 为 `POST /api/matching/manual-pair -> api/routes/matching.py::_parse_manual_pair_request() -> BillService.create_manual_transfer_pair() / create_manual_investment_pair() -> Database.create_manual_transfer_pair() / create_manual_investment_pair()`。`self-pair` 返回 `400`，账单不存在或不属于当前用户返回 `404`；当 `pairType=transfer` 时，重复/反向重复/任一账单已参与其他 transfer pair、已被 generic reject 持久化 suppress、或两条账单不满足 transfer-only 配对约束时返回 `409`；当 `pairType=investment` 时，冲突与候选约束会复用 investment/manual pair 规则并返回同样的 `409`。删除接口 `DELETE /api/matching/pairs/<id>` 当前会按 pair id 删除当前用户下的 manual pair：pair 不存在或不属于当前用户返回 `404`，非 manual source 返回 `409`；删除 transfer/manual pair 后会恢复对应 transfer candidates，删除 investment/manual pair 后则会恢复对应 investment candidates。列表接口 `GET /api/matching/pairs` 会按当前用户返回 `data{pairs[]}`，当前会读取 `bill_pair_links` 中已持久化的 manual pair 子集，并至少暴露 `transfer/manual` 与 `investment/manual` 两类 pair；每条 pair 仍附带 `id / pairType / source / leftBillId / rightBillId / createdAt / updatedAt` 以及 `leftBill / rightBill` 两侧正式账单的最小摘要，供后续规则中心配对总览或审查入口消费。该能力当前以 `bill_pair_links` + `bill_pair_feedback` + `bill_transfer_pair_suppressions` + `bill_investment_pair_suppressions` + `bill_learning_rule_suppressions` 作为最小持久化模型；其中 `bill_pair_feedback` 是 append-only 的 formal-bill feedback event stream，当前既承载 `transfer / investment` generic accept|reject 的 `candidate_id / action / payload_json` 写入，也通过 `GET /api/matching/bills/<bill_id>/feedback` 暴露 bill-scoped 审查视图，但仍不参与 pair 列表读模型。三类 suppressions 会在显式删除/批量删除账单、账户级删交易、去重、清空用户交易和清空用户业务数据时同步清理。
- 桌面端已入库交易详情当前已直接消费这条 historical matching 链路：运行时结构 map 为 `EditDialog.vue -> BillMatchingPanel.vue -> services.ts::getMatchingBillCandidates()/getMatchingBillFeedback()/acceptMatchingCandidate()/rejectMatchingCandidate()/deleteMatchingPair() -> GET /api/matching/candidates?billId=... / GET /api/matching/bills/<bill_id>/feedback / POST /api/matching/candidates/<candidate_id>/accept / POST /api/matching/candidates/<candidate_id>/reject / DELETE /api/matching/pairs/<id>`。该入口会在单条正式账单详情里展示 `linkedPair`、`transfer / investment / learning` 候选和 bill-scoped feedback 事件，并支持 generic accept / reject 以及清除当前 manual pair；当 formal-bill learning candidate accept 返回更新后的 `bill` 时，详情弹窗会同步刷新宿主交易字段；它当前是详情面板，不是新的独立规则中心页面。
- matching 域当前以 `bill_learning_rule_suppressions` 作为 formal-bill learning candidate 的 resolved-suppression 存储：`reject` 会写入 bill/rule suppress，`accept` 在成功应用 rule 后也会把该 bill/rule 标记为已解析，从而让后续 bill selector 读取不再重复暴露同一 learning candidate。该切片当前仍不引入 formal-bill learning pair link，而 `bill_pair_feedback` 也暂未扩到 learning family 或 preview family。
- 导入预览当前通过 `PUT /api/bills/import/v2/preview/<session_id>/update` 持久化行内编辑；当请求体额外携带 `responseMode="preview-item"` 时，响应会返回 `data{updated, previewItem}`，其中 `previewItem` 是已经过当前分类 / matching / 账户投影的单条预览行。导入预览里的学习建议草稿同步当前直接消费这条单行投影，不再需要为拿一条 learning candidate 再额外拉整会话 `GET /api/matching/candidates?sessionId=...`。
- 导入预览现已提供 `POST /api/bills/import/v2/preview-item/<preview_id>/transfer-decision` 作为转账建议决策接口：请求体除 `decision=accept|reject|clear` 外，还需要携带 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)` 作为防陈旧写保护；若预览状态在客户端读取后已变化，接口会返回 `409 Preview state changed, please refresh`。默认模式下接口仍返回刷新后的整批 `preview[]`；当请求体额外携带 `responseMode="preview-item"` 时，则改为返回 `data.previewItem` 单条刷新后的预览投影，避免为单行决策重取整批 preview。`matching.transfer` 当前除候选分数外，还会携带 `review_status / reviewed_type / suppressed` 供前端展示用户决策状态，且 `clear` 会恢复 `accept` 前被覆盖的 `preview_type / category / recurring` 展示字段。当前 `reclassify` / `confirm` 提交的 `preview_updates[]` 若已存在与该决策相关的本地类型/分类/周期编辑，会额外携带 `clear_transfer_decision=true`，用于清除已持久化的 transfer 决策反馈，避免 accepted / rejected 状态与手工编辑结果分叉。
- 导入预览现已提供 `PUT /api/bills/import/v2/preview-item/<preview_id>/recurring-match` 与 `DELETE /api/bills/import/v2/preview-item/<preview_id>/recurring-match` 作为单条定时候选确认/清除接口：两者都要求 `expectedState(sessionId, reviewStatus, previewType, categoryId, recurringId)` 做防陈旧写保护，`PUT` 额外要求 `recurringId`。默认模式下接口成功后返回刷新后的整批 `preview[]`；当请求体额外携带 `responseMode="preview-item"` 时，则返回 `data.previewItem` 单条刷新后的预览投影。它只更新 `bills_preview.preview_recurring_*` 与相关 `matching.recurring` 展示字段，并会清除与手工周期决策冲突的 transfer review feedback，但不会在 preview 阶段提前推进 `recurring_bills.next_date`。当前导入弹窗 `Check Data` 的单条 `Choose Scheduled Match` / `Clear Scheduled Match` 已改为直接走这组接口。
- `GET /api/matching/sessions/<session_id>/candidates` 中的 `recurring` candidate 当前会根据 preview 绑定状态投影 `status`：`preview_recurring_id` 已写入时返回 `confirmed`，清除绑定但仍保留候选数量时返回 `pending`，从而让 session 级读模型与预览表当前状态保持一致。
- 导入预览 `Check Data` 当前把交易类型职责与匹配信号职责拆开：类型列只负责展示/编辑 `preview_type` 以及与类型直接相关的编辑状态；`Signals` 列集中展示解析器来源、去重/转账、长期学习与定期交易等匹配信号，人工标注状态只通过行首编辑/标注入口和标注筛选表达，不再作为 `Signals` 列 chip 展示。投资识别不再依赖独立 preview keyword signal；它直接体现在 canonical category rules 命中的 `preview_type` / 分类结果，以及由该分类结果投影出的 `matching.investment` review 状态里。`parser / dedup` 继续作为上下文摘要显示，跨解析器来源会串联为类似 `匹配 | 民生银行 | 支付宝` 的提示。预览确认与重新分类请求结构不因这层展示消费而改变。
- 投资识别链路当前只保留两条运行时真相：一是导入预览/重新分类阶段先对普通银行结息、投资收益/分红/亏损等余额变化类流水做归一化，把它们转为普通收入/支出并清理同账户投资痕迹；二是真正的投资分类统一由 canonical `category_rules`（含 legacy 投资设置迁移后的规则表达式）驱动，命中投资类型分类规则时直接把账单类型提升为“投资”。`BillService` 不再在分类匹配之后额外跑一轮独立的投资关键词候选识别；历史正式账单 investment pairing 候选、manual pair 与 suppress/feedback 机制保持不变。
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
