# Bill Analyser 项目总览

Bill Analyser 是一个多来源账单导入、智能去重、自动分类、预算与统计分析全栈系统。当前运行态由 Rust Axum `bill_http_server` 独立承载，业务 API 统一通过 `REST /api/...` 进入后端；未知 `/api/...` 由 Rust 返回结构化 404。

## 运行态入口

- 后端主入口：`src/backend/http/bin/bill_http_server.rs`
- 后端导航图：`docs/backend-map.md`
- 静态阅读入口：`docs/backend-map.html`
- 前端工程：`src/web`
- 当前 API 主链：`REST /api/...`

本地启动要求 PostgreSQL 与 Weaviate 同时可达。PostgreSQL 是唯一业务数据库；Weaviate 是导入 learning recall 与派生向量索引的必需服务。

## 后端分层

- `src/backend/http`：Axum 路由、认证上下文、上传处理、response envelope、structured error 与各业务 route facade。
- `src/backend/core`：金额、时间、分类、统计、预算、导入、matching、LLM/OCR、认证、安全校验与运行态治理合同。
- `src/backend/db`：PostgreSQL schema scripts、SQLx pool、user-scope repository、导入 staging、auth、budget、matching、taxonomy/settings、backup metadata 与 vector outbox。
- `src/backend/parsers`：微信、支付宝、工商银行、农业银行、建设银行、民生银行等 dedicated parser，以及 `RawBill` 到 `StandardBill` 的统一标准化。

## 数据库与健康检查

默认本地运行态使用：

- `BILL_ANALYSER_DATABASE_BACKEND=postgres`
- `BILL_ANALYSER_POSTGRES_URL=postgres://bill_analyser:bill_analyser_dev@127.0.0.1:5432/bill_analyser`
- `BILL_ANALYSER_WEAVIATE_ENABLED=true`
- `BILL_ANALYSER_WEAVIATE_ENDPOINT=http://127.0.0.1:8088`

`/api/health` 的 details 暴露 PostgreSQL 配置状态、脱敏 URL、route repository backend、Weaviate ready 状态、脱敏 endpoint、API key 是否配置与 collection prefix。PostgreSQL 不可连接或 Weaviate ready probe 未通过时，整体 health 返回 `unhealthy`。

Rust HTTP 主入口在开始监听前会对配置的 PostgreSQL 运行 `src/backend/db/postgres/migrations` 当前迁移目录，确保后续运行态路由看到最新 schema；该目录属于 Rust DB crate 的运行态输入，部署/打包时必须随服务保留；未配置 PostgreSQL URL 时，仓储路由保持配置缺失状态并由 health/route error 暴露。

## API 与业务域

当前 HTTP route 覆盖登录、注册、邮箱验证、密码重置、refresh token、token session、logout、2FA、profile、user-data 统计与导出、交易清空、账户/分类/标签/模板主数据、分类规则、账户规则、设置包导入导出、账单列表与详情、手工交易、批量交易、账单导出、分类 quick-add/refresh、周期模板候选与绑定、matching pairs、净值快照、日历事件、预算 CRUD/导出/导入/执行/预测/快照、统计金额概览、分类统计、分类趋势、资产趋势、分类饼图、商户排行、Analyzer/insights、汇率读取与用户自定义汇率、备份文件 list/create/download/delete/verify/cleanup、备份任务与 cloud sync 元数据。

金额在数据库中使用明确的 minor units 字段；前端/API 的元/分转换只在 DTO 转换边界完成。预算 API 的 `amount` 继续使用元单位导出/导入并由仓储写入预算金额字段；交易模板 DTO 与设置包中的 `sourceAmount`、`destinationAmount` 保持分单位并写入 `transaction_templates.*_minor_units`，避免设置包导入时二次乘以 100。

## 导入链路

导入链路由 Rust 完成 parser-first 上传、JSON parse、session/source/template/standard-row staging、dedup、转账 materialization、分类规则、recurring/learning/LLM decision、账户规则匹配、preview page、preview update/reclassify、confirm 与 cleanup。多文件 staging 与 preview row 落库使用分块批量写入；preview 分页、筛选、排序和跨页选择由 PostgreSQL 在 `import_preview_rows` 上执行，前端只保留当前页交互草稿。

multipart 上传并行执行 dedicated parser 检测，每个文件必须且只能命中一个 dedicated parser 才会进入标准账单解析。stage2 phase precedence 为：同批/跨批 dedup 与转账 materialization 先形成预览基底，分类规则与内置分类兜底先确定类型/分类并写入 `category_id`，recurring 与可自动应用的 learning projection 可继续改写类型、分类或显式账户，账户规则最后消费稳定后的预览类型、转账/投资上下文和仍为空的账户字段。确定性规则链未命中时，通过 Weaviate 召回派生 learning 建议。Weaviate metadata 本身不会触发自动改写。

导入 staging 写入前统一规范化账单日期文本：常规日期/时间、银行 Excel 日期序列，以及“日期 + 小数日时间”会转换为标准 `YYYY-MM-DD HH:MM:SS` 文本再进入 PostgreSQL 时间字段。

账户识别以 `account_rules` 为权威；前端账户 DTO 不包含别名字段，账户规则以账户、表达式、优先级、启停状态为当前合同。`account_rules` 当前迁移后 schema 不再包含旧 `account_role_scope`、`transaction_type_scope` 与 `field_scope` 持久化列；旧 payload、query 或 settings bundle 中携带这些字段时只在 API/导入边界产生兼容 warning 并被忽略。导入运行时按稳定后的账单类型、账户角色和上下文字段包决定匹配目标；非正则账户规则只匹配规范化后的完整 token，避免“本行”误匹配“本行POS”等渠道文本。账户 API 在 DTO 边界把恢复数据或旧式数据中缺失的账户类型、账户分类归一成当前前端分类合同；缺失分类会优先保留显式值，再按账户名称、图标和旧式类型推断，不用统一现金兜底覆盖已恢复账户。

## 分类与规则中心

分类识别使用 `category_rules` 规则表达式；分类规则列表只暴露能投影出非空表达式的规则，避免旧恢复行或坏数据在规则中心显示为空匹配式。账户识别使用 `account_rules` 表和同一表达式匹配器模型，账户规则 API 和设置包导出不再传播旧 role/type/field scope 字段；旧 payload 或旧 bundle 中的 scope 字段会被忽略并返回兼容 warning。REST API 覆盖账户规则 list/create/update/delete/reorder/test，以及分类规则 list/create/update/delete/reorder/defaults/test。设置包按当前 PostgreSQL 主链导出并导入/upsert `accounts`、`transactionCategories`、`transactionTags`、`transactionTemplates`、`scheduledTransactions`、`categoryRecognitionRules` 与 `accountRecognitionRules`；`transactionTemplates` 和 `scheduledTransactions` 在账户、分类、标签引用重映射完成后写入 `transaction_templates`，有效导出不再以 unsupported section warning 跳过。

桌面规则中心的“规则配置”包含分类识别、账户识别和周期识别三个二级页；分类识别按一级分类聚类展示，同一分类目标的多条规则表达式在同一行内分行呈现；账户识别按账户主分类、父账户/子账户分组展示，主分类行只呈现图标和规则数量，同一账户下的多条规则表达式在同一行内分行呈现，不重复显示规则名、优先级、匹配次数等运行态元数据；移动端通过 `/account/rules` 提供同样分组后的账户规则列表与紧凑编辑/测试入口。

## Matching

正式账单的转账候选与导入配对保持同一核心约束：同日、5 分钟内、金额绝对值在 1 分容差内且方向相反、来源账户不同，并排除已配对或已 suppressed 的账单。接受候选会合并或删除对应账单并写入审计；拒绝候选会写入 suppression，避免同一对账单再次提示。

规则中心的配对总览展示转账配对和重复配对；投资相关识别规则保留在分类/规则治理链路中。

## 预算与统计

预算按月/季/年层级同步，删除主预算或最后一个子预算时会清理派生父周期预算。预算 CRUD、导出、导入、执行、预测和快照按同一层级、筛选、账户/标签上下文与名称更新语义执行，并把 API 元单位金额写入 minor units 列。统计链路由 Rust 读取账户、分类、汇率和资产趋势数据，并保持前端图表契约。

## 认证与备份

认证运行态校验 Bearer access token 签名、过期时间和 PostgreSQL token session。登录、注册、邮箱验证、密码重置、refresh、logout、2FA、profile、cloud settings、external auth、user-data statistics/export/clear 直接读写 PostgreSQL 表。敏感动作通过当前密码、操作密码或签名 step-up token 校验，并写入认证或业务审计。

备份运行态负责本地 zip/Fernet 文件 I/O、公开名生成、文件 list/create/download/delete/verify/cleanup、job list/save 与 cloud sync 元数据；记录、任务和审计元数据写入 `backup_records`、`backup_jobs`、`backup_audit_logs`。

## LLM/OCR

LLM 临时配置保存在 Rust 进程内 user-scoped map，saved config、候选项、memory event 与 annotation sample 由 PostgreSQL 迁移表承载并按 API key 规则脱敏。provider 生成保留 allowlist/SSRF 防护、响应体上限、候选截断和 rate limit。OCR recognition 默认 disabled，配置后可通过 Tesseract、本地 JSON OCR 或 LLM vision provider 返回结构化交易草稿。

## 文档入口

- [后端导航图](backend-map.md) — Rust 后端分层、请求生命周期、导入管线、repository 数据流和验证矩阵
- [总体架构](overview-architecture.md) — Rust 后端、前端、PostgreSQL 与 Weaviate 主链
- [后端模块](overview-backend.md) — HTTP/Core/DB/Parser 责任边界
- [API 路由](overview-api-routes.md) — `/api/...` route modules 和合同约束
- [导入链路](overview-import.md) — 三阶段导入、preview、learning、LLM/OCR
- [导入全链路验收场景](import-full-chain-scenarios.md) — parser-first、stage2、preview、confirm、PostgreSQL/Weaviate 必需运行态矩阵
- [数据库与数据流](overview-database.md) — repository、事务、user-scope 和 staging 生命周期
- [Weaviate derived index](weaviate-derived-index.md) — 必需向量派生索引配置、bootstrap、outbox 和 rebuild
- [Matching 域](overview-matching.md) — transfer/learning/recurring 配对候选
- [统计与汇率](overview-statistics.md) — 统计主链、汇率 REST 和用户数据管理
- [认证与安全](overview-auth-security.md) — 认证、2FA、token、backup、step-up 和审计
- [测试结构](overview-testing.md) — Rust、前端和治理测试基线

## 验证基线

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35
```

```powershell
Set-Location src\web
npm run lint
npm run test:coverage
npm run build
```
