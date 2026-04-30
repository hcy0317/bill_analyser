# Bill Analyser 后端模块分布

## 3.1 API 路由模块（`src/bill_analyser/api/routes/`）
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

## 3.1.1 API 适配器出口（`src/bill_analyser/api/adapters/`）
- 中性实现模块：`transaction_adapter.py`、`account_adapter.py`、`category_adapter.py`
- legacy `v1_*` wrapper 文件已移除，避免新增代码继续耦合旧文件名
- 当前真实实现统一收口到中性模块；新增代码、路由与测试应只直接引用这些中性模块
- 静态回归：`tests/new_ui/test_no_direct_legacy_adapter_usage.py` 会阻止重新引入 `V1*Adapter` / `V1ResponseBuilder` 旧命名、直接导入 `v1_* adapter` 模块，或重新提交这些 wrapper 文件

## 3.2 核心业务模块（`src/bill_analyser/core/`）
- `db.py`：薄 `Database` façade；对外维持统一导入入口，内部按 mixin 组装数据库能力
- `db_runtime.py` / `db_shared.py` / `db_time.py`：数据库连接生命周期、共享请求/分组数据结构、UTC 时间与缓存辅助
- `db_schema.py` + `db_schema_core.py` + `db_schema_users_security.py` + `db_schema_templates_imports.py`：schema 初始化与迁移编排；核心业务表、用户安全表、模板/导入相关表分别维护
- `db_bills.py` / `db_categories.py` / `db_accounts.py` / `db_tags.py` / `db_templates.py`：账单、分类、账户、标签、模板域的 CRUD、批量操作与查询辅助
- `db_users_auth.py` / `db_user_data.py` / `db_audit_backup.py`：用户与会话、2FA 与应用设置、用户数据管理、审计/备份域持久化逻辑
- `db_budgets_core.py` / `db_budgets_execution.py` / `db_budgets_forecast.py`：预算主数据、分类上下文与分组 helper，以及执行统计 / 历史 / 预测 / 导入导出查询；预算分类上下文会把历史 `categories.type=1` 归一为当前支出类型 `3`，避免旧分类预算在执行统计和预算列表中被类型过滤漏掉
- `db_budgets_reporting.py`：预算 reporting 兼容聚合层；运行时通过它组合 execution/history 与 forecast/import/export 两个预算域 mixin
- `db_import_configs.py` / `db_import_sessions.py` / `db_import_preview.py` / `db_import_learning.py` / `import_learning/`：导入模板配置、三阶段会话、预览编辑/确认、长期学习 durable corpus、exact/manual 学习规则、session-scoped dry-run suggestions，以及基于 dataset snapshot / active model registry 的轻量双头学习模型训练与推理
- `bill_service.py`：导入主流程编排（含 v2 三阶段导入）
- `smart_dedup.py`：智能去重引擎主实现；导入批次内按"完全重复 → 平台-银行去重 → 转账配对 → 相似去重 → 分账去重"的顺序处理，含数据库对比时再追加数据库重复检测与跨批次转账配对。平台-银行去重优先保留支付平台账单；同号平台/银行候选直接按重复处理，异号候选只有在没有明确转账意图且具备文本重复证据时才先于转账配对吸收，避免把真实转账错误归并为平台-银行重复。
- `category_engine.py`：分类规则匹配式解析与分类匹配（含类型过滤与预编译优化）；运行时仅从 `category_rules` canonical source 加载规则，不再回退到 `categories.keywords`。`rule_expression` 后端兼容旧 `OR:a|b&AND:c&NOT:d`，正式语法由 `OR={...}`、`AND={...}`、`NOT={...}`、`REGEX={...}` 子句组成：`+` 表示 AND，`/` 表示同一表达式内 OR，`|` 表示表达式级 OR，`×` 或 `NOT` 表示 AND NOT；多层括号按 AST 优先级执行。前端共享规则构建器把一条 canonical 分类规则呈现为"多个表达式 + 表达式内多个规则块"，支持多层括号、连接符与 OR/AND/NOT 标签分离展示，并在同一表达式内保持 `/` 连接块同组展示；迁移旧关键词会转义分隔符与括号，避免特殊字符关键词被拆成错误语义。分类规则运行时匹配顺序由 `categories.priority`、`category_id` 与规则 id 的稳定顺序决定，`category_rules.priority` 仅作为兼容元数据保留；加载规则时会把历史 `categories.type=1` 归一为支出类型，匹配结果会保留 canonical category id/type，无法解析到真实分类时 fail closed，不生成同名虚拟分类。
- `exchange_rate_providers.py`：多汇率提供者聚合
- `budget.py`：历史预算管理兼容层，保留 `BudgetManager` 供旧 CLI / 旧测试路径复用；当前 CLI 预算报告主链直接走 `Database.get_budget_execution_details()`
- `sync.py`：同步相关编排
- `analyzer.py`：统计分析、聚合与图表/报表数据生成主服务，仍是统计域的活跃运行时代码
- `report.py` / `utils/report_export.py`：`core.report` 保留旧导入路径兼容壳，真实 PDF / Excel / HTML 报告导出实现已归并到 `utils.report_export`
- `smart_dedup.py`：智能去重引擎主实现，仍在导入主链中承担平台银行配对、相似去重与分账去重
- `smart_dedup_v641_backup.py` 等历史版本备份文件已从运行时代码树移除

## 3.2.1 Database façade 关系
- 外部调用方（API 路由、服务、测试）继续只从 `src/bill_analyser/core/db.py` 导入 `Database`
- `Database` 通过多继承顺序组合各域 mixin：预算 reporting → 导入 preview / session / config / learning → 用户数据 / 认证 → 模板 / 标签 / 账户 / 分类 / 账单 → 审计备份 → schema → runtime
- schema 初始化由 `DatabaseSchemaMixin.init_db()` 统一编排，并在运行时通过 `DatabaseRuntimeMixin` 提供连接、路径重定向、缓存与上下文管理
- 这种结构保持了公共 API 稳定，同时把预算域、导入链路、账户/标签/模板、认证安全等高耦合逻辑拆到独立文件维护

## 3.3 解析器模块（`src/bill_analyser/parsers/`）
- 已有解析器：`wechat.py`、`alipay.py`、`icbc.py`、`abc.py`、`ccb.py`、`cmbc.py`
- 工厂入口：`factory.py`（自动识别并分发）
- `ParserBase` 当前除标准账单字段外，还会生成扁平 `parser_tags` 标签数组；三阶段导入临时表分别通过 `bills_parser_template.parser_tags_json` 与 `bills_preview.preview_parser_tags_json` 持久化这些解析器元标签
- 历史 `parsers_old/` 目录与 `*_parser.py` / `csv_parser.py` / `excel_parser.py` 兼容 shim 已移除；运行态仅保留当前工厂与现行解析器实现
- 外部脚本如果仍引用旧 shim 导入路径，应迁移到 `factory.py` 或对应的现行 parser 模块
- 标准解析样本统一放在 `tests/fixtures/import_samples/`，供 parser 单测与真实样本回归复用
