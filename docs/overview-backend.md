# Bill Analyser 后端模块分布

## 3.1 API 路由模块（`src/bill_analyser/api/routes/`）
- `auth/`：认证同名 package；按 session、registration/password、profile/external-auth、tokens、2FA、step-up、user-data 等功能域拆分，`auth.bp` 和既有 route URL/method 保持稳定
- `bills/`：账单同名 package；按 CRUD/pictures、通用导入解析、导入配置、导入学习、reconciliation、v2 session/preview/decision 等功能域拆分，`bills.bp` 和 `/api/bills/*` 契约保持稳定
- `matching/`：matching 同名 package；导入会话级 matching candidate 投影、preview family generic accept/reject、历史正式账单后配对与 pair 列表接口，以及规则中心 investment pairing settings 的 canonical REST 读写接口
- `rules.py`：规则中心 legacy 兼容聚合口；当前 overview 只聚合 learning rules、category rule count 与 recurring rules，不再把 legacy `category_keywords` 作为运行时总览来源
- `accounts/`：账户同名 package；账户 CRUD、余额同步、显示顺序与账户交易迁移/清空接口
- `categories/`：分类同名 package；分类列表/树、创建更新删除、移动、统计、导入导出与规则维护接口
- `category_rules.py`：分类规则的 canonical CRUD / migrate / defaults / test 接口；category rules 是当前分类规则体系的正式入口，`POST /api/category-rules/defaults` 会幂等补齐内置日常分类和分类识别规则
- `tags.py`：标签管理与关联
- `budgets/`：预算同名 package；预算 CRUD、执行统计、历史、预测、导入导出接口
- `statistics/`：统计同名 package；统计总览、趋势、分类/商户分析、资产趋势与汇率接口
- `templates.py`：模板管理
- `settings_bundle.py`：设置 JSON 包导入导出；统一覆盖账户、交易分类、交易标签、交易模板、定时交易、分类识别规则、LLM 配置与 OCR 配置，并提供各页面使用的 section-scoped export / preview import / import，导入只做非破坏性 upsert；LLM/OCR 单 section 导出必须校验当前登录密码
- `backup/`：备份同名 package；备份文件、恢复、任务与清理接口
- `llm/`：LLM 同名 package；配置、导入会话分析、候选审核、预览推荐与 memory 事件接口
- `receipt_ocr.py`：小票/支付截图 OCR REST 蓝图，继续提供 `POST /api/ml/receipt-recognition` 与 `GET/PUT /api/ml/receipt-recognition/config`

## 3.1.1 API 适配器出口（`src/bill_analyser/api/adapters/`）
- 中性实现模块：`transaction_adapter.py`、`account_adapter.py`、`category_adapter.py`
- legacy `v1_*` wrapper 文件已移除，避免新增代码继续耦合旧文件名
- 当前真实实现统一收口到中性模块；新增代码、路由与测试应只直接引用这些中性模块
- 静态回归：`tests/new_ui/test_no_direct_legacy_adapter_usage.py` 会阻止重新引入 `V1*Adapter` / `V1ResponseBuilder` 旧命名、直接导入 `v1_* adapter` 模块，或重新提交这些 wrapper 文件

## 3.2 核心业务模块（`src/bill_analyser/core/`）
- `db.py`：薄 `Database` façade；对外维持统一导入入口，内部按 mixin 组装数据库能力
- `database/runtime.py` / `database/shared.py` / `database/time.py` / `database/encryption.py`：数据库连接生命周期、共享请求/分组数据结构、UTC 时间、缓存辅助与 SQLCipher 配置
- `database/schema/`：schema 初始化与迁移编排；`schema/core/`、`schema/users_security.py`、`schema/templates_imports/` 分别维护核心业务表、用户安全表、模板/导入相关表，并按业务表、matching、预算/汇率、索引和迁移拆分 SQL 初始化块
- `database/bills/` / `database/categories/` / `database/category_rules/` / `database/accounts/` / `database/tags/` / `database/templates/` / `database/settings_bundle/`：账单、分类、分类规则、账户、标签、模板域的 CRUD、批量操作、设置包导入导出与查询辅助；账户和设置包域已按读取/变更/余额、section export/import/upsert/resolution 拆分，模板域由 `templates/__init__.py` 保留 `DatabaseTemplatesMixin` 门面，真实实现按 `crud.py`、`recurring.py`、`schedule.py`、`serialization.py` 与 `mixin.py` 维护。
- `database/users/auth/` / `database/users/data/` / `database/audit_backup/`：用户与会话、2FA 与应用设置、用户数据管理、审计/备份域持久化逻辑
- `database/budgets/core/` / `database/budgets/execution/` / `database/budgets/forecast/` / `database/budgets/reporting/`：预算主数据、分类上下文与分组 helper，以及执行统计 / 历史 / 预测 / 导入导出查询；预算分类上下文会把历史 `categories.type=1` 归一为当前支出类型 `3`，避免旧分类预算在执行统计和预算列表中被类型过滤漏掉。一级/二级分类预算与月度/季度/年度预算都按父子预算关系上卷：子集总额不超过父级时保留父级金额，超过时父级金额提升为子集总和；历史预算读取会在无账单支出时按需返回预算行，并在支出/投资类型之间保持隔离。
- `database/imports/configs/` / `database/imports/sessions/` / `database/imports/preview/` / `database/imports/learning/` / `import_learning/`：导入模板配置、三阶段会话、预览编辑/确认、长期学习 durable corpus、exact/manual 学习规则、session-scoped dry-run suggestions，以及基于 dataset snapshot / active model registry 的轻量双头学习模型训练与推理
- `database/matching/` / `database/reconciliation/` / `database/llm/candidates/`：正式账单 matching、导入 reconciliation、LLM candidate/memory/preview apply-review 持久化 package；内部按 settings/feedback/suppression/candidates/actions/manual pairs、projection/actions/persistence、candidate CRUD/feedback/memory/preview 等职责拆分
- `database/llm/config/` / `database/recurring_suggestions/`：LLM provider 配置与高级参数、周期识别候选持久化；对外继续导出原 mixin / normalization helper 名称
- `bills/`：账单服务核心域包；公共入口 `core.bills.BillService` 由 `bills/__init__.py` 导出，真实实现按 `bills/service_parts/` mixin 组合，分别维护 legacy 导入、v2 三阶段导入、预览投影/分页、账户匹配、学习规则/信号、matching 读写、preview 决策与 reclassify 等域。
- `smart_dedup/`：智能去重引擎 package；公共导入仍由 `core.smart_dedup` 输出，内部按模型、标准化、精确重复、平台-银行、分组/相似、转账、reconciliation 与数据库重复检测拆分。导入批次内仍按"完全重复 → 平台-银行去重 → 转账配对 → 相似去重 → 分账去重"顺序处理，含数据库对比时再追加数据库重复检测与跨批次转账配对。
- `category_engine/`：分类规则匹配 package；公共导入仍由 `core.category_engine` 输出，内部按表达式 AST/转义、预编译规则、匹配器与 `CategoryEngine` 规则加载/缓存拆分。运行时仅从 `category_rules` canonical source 加载规则，不再回退到 `categories.keywords`。`rule_expression` 后端兼容旧 `OR:a|b&AND:c&NOT:d`，正式语法由 `OR={...}`、`AND={...}`、`NOT={...}`、`REGEX={...}` 子句组成：`+` 表示 AND，`/` 表示同一表达式内 OR，`|` 表示表达式级 OR，`×` 或 `NOT` 表示 AND NOT；多层括号按 AST 优先级执行。分类规则运行时匹配顺序由 `categories.priority`、`category_id` 与规则 id 的稳定顺序决定，无法解析到真实分类时 fail closed，不生成同名虚拟分类。
- `default_category_seed.py`：内置日常生活分类与规则种子，按餐饮、食品日用、居住家庭、交通出行、医疗健康、教育成长、娱乐休闲、收入、转账等主域提供非破坏式补全；规则写入 canonical `category_rules` 并按名称幂等跳过。
- `ai/ocr/provider.py` / `ai/ocr/service.py` / `ai/ocr/payment_screenshot_parser.py`：OCR provider、识别编排与支付截图文本解析的统一实现域；当前只承诺支付宝与微信支付截图中的金额、时间、商户/备注和平台标识，不做 embeddings 存储或后台全量图片处理。
- `exchange_rate_providers/`：多汇率 provider package；公共导入仍由 `core.exchange_rate_providers` 输出，内部按 provider 基类、解析辅助、中国 provider、全球 provider 与 manager 拆分。
- `ai/llm/provider.py` / `ai/llm/prompts.py` / `ai/llm/learning_service/`：LLM provider、prompt 构造与学习服务的统一实现域；内部按 provider 调用、高级参数、限流、导入会话分析、规则合成与预览推荐拆分，用户级 active config 与高级参数保持实例/请求隔离。
- `budgets/manager.py` / `budgets/execution_summary.py`：预算核心域包；保留 `BudgetManager` / `BudgetStatus` 供 CLI、API 与测试复用，预算执行汇总 helper 独立维护；当前 CLI 预算报告主链直接走 `Database.get_budget_execution_details()`
- `investment/matching.py` / `investment/settings.py`：投资识别与投资关键词设置核心域包；matching、导入预览与分类规则迁移共享同一套评分、默认关键词与序列化 helper。
- `sync.py`：同步相关编排
- `analyzer.py`：统计分析、聚合与图表/报表数据生成主服务，仍是统计域的活跃运行时代码；图表生成公共入口继续由 `utils/charts.py` 导出 `ChartGenerator` / `generate_all_charts`，真实实现按趋势、分类、对比、排行、预算、热力图和 dashboard 拆分到 `utils/charting/`。
- `report.py` / `utils/report_export.py`：`core.report` 保留旧导入路径兼容壳，真实 PDF / Excel / HTML 报告导出实现已归并到 `utils.report_export`
- `smart_dedup_v641_backup.py` 等历史版本备份文件已从运行时代码树移除

## 3.2.1 Database façade 关系
- 运行态业务调用方优先只从 `src/bill_analyser/core/db.py` 导入 `Database`；需要直连数据库 helper 或 mixin 的测试/支持模块通过 `src/bill_analyser/core/database/**` 导入
- `Database` 通过多继承顺序组合各域 mixin：预算 reporting → 导入 preview / session / config / learning → 用户数据 / 认证 → 设置包 / 模板 / 标签 / 账户 / 分类 / 账单 → 审计备份 → schema → runtime
- schema 初始化由 `DatabaseSchemaMixin.init_db()` 统一编排，并在运行时通过 `DatabaseRuntimeMixin` 提供连接、路径重定向、缓存与上下文管理
- 这种结构保持了公共 API 稳定，同时把预算域、导入链路、账户/标签/模板、认证安全等高耦合逻辑收敛到 `core/database/**` 的语义 package 维护；`core/` 根层只保留 `db.py` 门面

## 3.3 解析器模块（`src/bill_analyser/parsers/`）
- 已有解析器：`wechat.py`、`alipay.py`、`icbc.py`、`abc.py`、`ccb.py`、`cmbc.py`
- 工厂入口：`factory.py`（自动识别并分发）
- `ParserBase` 当前除标准账单字段外，还会生成扁平 `parser_tags` 标签数组；解析器标签的规范化、推断与 JSON 序列化契约由中性 `import_contracts/parser_tags.py` 提供，`parsers/parser_tags.py` 仅保留公共兼容导出；三阶段导入临时表分别通过 `bills_parser_template.parser_tags_json` 与 `bills_preview.preview_parser_tags_json` 持久化这些解析器元标签
- 历史 `parsers_old/` 目录与 `*_parser.py` / `csv_parser.py` / `excel_parser.py` 兼容 shim 已移除；运行态仅保留当前工厂与现行解析器实现
- 外部脚本如果仍引用旧 shim 导入路径，应迁移到 `factory.py` 或对应的现行 parser 模块
- 标准解析样本统一放在 `tests/fixtures/import_samples/`，供 parser 单测与真实样本回归复用
