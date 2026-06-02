# Bill Analyser 项目总览

Bill Analyser 是一个多来源账单导入、智能去重、自动分类、预算与统计分析全栈系统。当前运行态已经完成 Rust-only 切换：`bill_http_server` 是唯一 HTTP 服务入口，默认监听 `BILL_ANALYSER_HTTP_BIND=127.0.0.1:5000`，业务 API 通过 Rust Axum `REST /api/...` 进入后端；未知 `/api/...` 返回 Rust 侧结构化 404。

## 运行态入口

- 后端主入口：`src/backend/http/bin/bill_http_server.rs`
- 后端导航图：`docs/backend-map.md`
- 静态阅读入口：`docs/backend-map.html`
- 前端工程：`src/web`
- 当前 API 主链：`REST /api/...`
- 运行态日志：Rust HTTP 服务使用 `tracing`，默认过滤等级为 `info`；开发环境可通过 `RUST_LOG` 打开各 crate 或模块的 `debug` span。业务日志字段只记录功能域、操作和不敏感内部 ID/计数，不记录请求体、文件名、交易描述、prompt、token 或密钥。

## 后端分层

- `src/backend/http`：Axum 路由、认证上下文、上传处理、response envelope、structured error 和各业务 route facade。
- `src/backend/core`：金额、时间、分类、统计、预算、导入、matching、LLM/OCR、认证与迁移治理等共享业务合同。
- `src/backend/db`：SQLite legacy 迁移输入、PostgreSQL schema/migration、事务 helper、user-scope repository、导入 staging、auth、budget、matching、taxonomy/settings 等仓储模块；同时提供 PostgreSQL 权威库 schema foundation、SQLite/Postgres repository runtime provider。当前正常 HTTP 运行态目标是 PostgreSQL authority + 必需 Weaviate；SQLite 只保留为离线迁移来源、隔离测试 fixture 或显式 migration tooling，不能作为业务 route fallback。
- `src/backend/parsers`：微信、支付宝、工商银行、农业银行、建设银行、民生银行等 dedicated parser，以及 `RawBill` 到 `StandardBill` 的统一标准化。

详细的代码阅读路径、请求生命周期、导入管线、repository 数据流和验证矩阵见 `docs/backend-map.md`。

## 数据库运行配置

默认本地运行态使用 `BILL_ANALYSER_DATABASE_BACKEND=postgres`、`BILL_ANALYSER_REQUIRE_POSTGRES_AFTER_CUTOVER=true`、`BILL_ANALYSER_POSTGRES_URL` 和必需 Weaviate endpoint。未配置 Postgres URL 时，Rust 配置与 `start_backend.ps1` 使用本地 compose 默认 `postgres://bill_analyser:bill_analyser_dev@127.0.0.1:5432/bill_analyser`，一键启动器在默认端口被其他进程占用时会选择可用本地端口并把对应 URL 传给后端。业务 route 不能再打开 SQLite runtime 作为 fallback；即使直接通过环境变量设置 `BILL_ANALYSER_DATABASE_BACKEND=sqlite` 与 `BILL_ANALYSER_REQUIRE_POSTGRES_AFTER_CUTOVER=false`，HTTP 仓储边界也会报告 `sqlite_legacy_disabled` 并拒绝业务 SQLite。若 backend 不是 Postgres、Postgres URL 缺失或仓储尚未接管，health 会报告 unhealthy 而不是回退。

`/api/health` 的 details 暴露 `database_backend`、`route_repository_backend`、`postgres_cutover_status`、`postgres_configured`、`postgres_url_redacted`、`migration_mode`、`migration_status`、`weaviate_required`、`weaviate_status`、`weaviate_endpoint_redacted`、`weaviate_api_key_configured`、`weaviate_collection_prefix`、`require_postgres_after_cutover` 与 `legacy_sqlite_runtime_allowed`。健康信息只显示脱敏 Postgres URL 和不含密钥的 Weaviate endpoint；`route_repository_backend=postgres_authority` 与 `postgres_cutover_status=complete:postgres_authority` 表示正常 HTTP 仓储边界已以 PostgreSQL 为权威，只有在 Weaviate ready probe 为 `healthy` 时整体 health 才会返回 `ok`；`route_repository_backend=sqlite_legacy_disabled` 表示直接配置 SQLite 也不能进入正常 HTTP 业务仓储；`route_repository_backend=sqlite_legacy` 只会出现在隔离测试 fixture 这类显式 test-only 访问中，不能让正常 HTTP health 变成 `ok`；`postgres_required_after_cutover` 表示配置仍不满足 Postgres 权威要求。迁移状态仍是占位观测字段；Weaviate 运行态默认启用本地 endpoint，并通过 `/v1/.well-known/ready` 报告 `healthy` 或 `degraded:<reason>`。任何正常 HTTP 业务路径若尝试打开 SQLite runtime，都会被 repository boundary 以 503 fail-closed 拒绝，不会静默回退。

PostgreSQL 迁移工具入口是 `bill_sqlite_to_postgres_migrate`，支持 `dry-run`、`export`、`import-check` 和事务性 `import`。它从 SQLite 读取 legacy 表，校验必需列，生成确定性 checksum，并把账户别名转换成目标 `account_rules` payload；`import` 通过 SQLx 写入 PostgreSQL、记录 `migration_audit_events`、修正 identity 序列，不改写 SQLite，也不把业务 repository 切到 Postgres。单账号业务数据恢复使用独立入口 `bill_postgres_account_recovery`，按显式 SQLite 源用户和显式 Postgres 目标用户生成 pseudonymous sensitive manifest；`apply` 要求 manifest id、目标用户 id/username/email 确认、预期源数据形状匹配和 git-ignored snapshot 目录，并在单个目标用户事务中清空/重建 accounts、categories、tags、bills、budgets、category_rules 和由 legacy aliases 转换出的 `account_rules`，同时把 legacy aliases 保留在账户 metadata 供账户设置页继续展示，并保留 users、认证、2FA、外部登录、parser templates、transaction templates、操作密码、备份/云端/LLM/OCR 凭据和审计状态。若恢复目标用户需要沿用 SQLite 源用户登录密码，独立 `sync-auth` 模式只同步源 `users.password_hash` 并清理目标登录锁定 metadata，不重建业务数据。操作步骤见 [PostgreSQL migration tooling](postgres-migration.md)。

PostgreSQL HTTP 接管面已覆盖登录、注册、邮箱验证、密码找回/重置、refresh token、token session 列表/生成/撤销、logout session 撤销、2FA status/verify/enable/disable/recovery、Profile GET/更新/头像/cloud settings/profile verification resend/external auth list+unlink、Bearer JWT 解析、user-data 统计、CSV/JSON 导出、交易清空和全量清空、账户/分类/标签/模板主数据列表、账户 CRUD/排序、分类 CRUD/批量创建/导入/导出/排序/统计、标签 CRUD/批量创建/排序、模板 CRUD/排序、设置包账户/分类/标签/分类规则/账户规则预览导入、导入和导出、交易列表/按月列表/详情读取/CSV/XLSX 导出、对账单汇总、分类 quick-add/refresh、周期模板候选/绑定/解绑、手工交易新增/更新/删除、批量新增/批量更新/批量删除与 legacy modify/delete 兼容入口、统计金额概览、分类统计、分类趋势、资产趋势、分类饼图、商户排行、Analyzer/insights 读取、汇率读取与用户自定义汇率设置、手工 matching pairs 列表、净值快照、日历事件与周期建议列表/检测/接受/拒绝、预算列表、预算 CRUD、预算导出、预算导入、预算执行、预算预测、预算历史读取和预算历史快照，以及备份文件列表/创建/下载/删除/恢复/校验/cleanup、备份任务列表/保存和 cloud sync 审计元数据。这些路径在 `database_backend=postgres` 时直接使用 PostgreSQL pool，并保持现有前端 DTO 投影；账单、账户、预算金额字段在 PostgreSQL 中显式存分，手工交易和批量交易写入 `bills.amount_cents` 并按收入/支出/转账语义增量同步 `accounts.balance_cents`，分类 quick-add 写入 `categories.metadata.keywords`，账单分类 refresh 使用 PostgreSQL `category_rules` 回写 `bills.category_id` 与 `standard_payload` 分类字段，周期模板匹配使用 `transaction_templates(template_type=2)` 和 `bills.standard_payload.created_from_recurring`，预算历史快照金额写入 `budget_history.*_amount_cents` 分单位列，模板金额字段存精确整数 minor units，周期建议金额写入 `recurring_suggestions.amount_cents` 分单位列，备份 runtime 的文件 I/O 仍由 Rust zip/Fernet 执行，备份记录、任务和审计元数据写入 `backup_records`、`backup_jobs`、`backup_audit_logs`，返回前端前再转换到原 API 形态。user-data clear 在 PostgreSQL 模式下通过签名 step-up token、当前密码、环境操作密码或 user-scoped `settings.operation_password` 校验敏感动作，删除用户业务表数据并写入 `business_audit_events`，不会打开 SQLite session fallback。

Weaviate 派生索引通过 `BILL_ANALYSER_WEAVIATE_ENABLED`、`BILL_ANALYSER_WEAVIATE_ENDPOINT`、`BILL_ANALYSER_WEAVIATE_API_KEY`、`BILL_ANALYSER_WEAVIATE_COLLECTION_PREFIX`、timeout、retry、batch size 和 vector dimensions 配置，运行态默认启用 `http://127.0.0.1:8088`。`start_backend.ps1` 会把 Weaviate 作为必需服务预检；`bill_weaviate_derived_index` CLI 提供 health、bootstrap、process-outbox 和 rebuild；`vector_outbox_events` 在 PostgreSQL 中记录待同步事件，Weaviate 只保存自供向量、特征副本和 metadata，可按用户从 PostgreSQL 权威学习特征表重建。导入 stage2 在确定性规则链后执行向量召回，召回请求按 user/schema/type/rule-state metadata 过滤；Weaviate metadata 本身不能触发自动改写。

## 关键业务链路

### 导入

导入链路由 Rust runtime 完成 parser-first 上传、JSON parse、session/source/template/standard-row staging、dedup、转账 materialization、分类规则与账户规则匹配、recurring/learning/LLM decision、preview page、preview update/reclassify、confirm 和 cleanup。multipart 上传会并行执行 dedicated parser 检测，每个文件必须且只能命中一个 dedicated parser 才会进入标准账单解析；未命中或多 parser 冲突会作为 unmatched 文件返回，并携带 `parser_decision` 证据。混合来源上传按文件保留 parser id / tags / parser decision，并把每个来源写入 `import_sources`，把标准化账单写入 `import_standard_rows`，供后续 duplicate、transfer 和学习链路复用。同批导入会先按秒级时间窗口、同向金额、方向和文本证据折叠重复账单，合并交易对方/支付方式/描述并把 parser/source 链写入 preview feedback 与 `import_decision_groups`；同批转账按秒级时间窗口、同额反向金额和不同来源配对，以支出账单为基底合并交易对方/支付方式/描述，把支出/收入原始字段保存在 transfer source chain，并写入 same-batch transfer decision group。与正式 `bills` 命中的历史重复要求金额方向和文本证据，会把历史账单 materialize 成预览行；与正式账单命中的历史转账按同日、时间容差、同额反向金额和不同来源配对，以支出侧为基底生成 `transfer_cross_batch` 预览行。历史重复和历史转账都会标记“将改写/合并历史账单”，写入 `import_history_materializations` 及对应 historical decision group；confirm 对这些破坏性历史操作要求前端回传可见操作标记、history bill id/version、operation id 和 acknowledgement token，验证通过后在事务中更新历史账单或创建转账基底并删除被合并历史侧，写入 `import_confirm_operations` 审计并同步账户余额。桌面 Check Data 和移动端导入预览都会在信号列/信号条展示 parser、重复、转账、历史改写、learning 与 LLM 信号，支持按信号筛选和可操作建议的接受/拒绝；确认导入前会把选中预览行的历史改写操作整理成 acknowledgement payload。Check Data 首屏读取分页 preview page；stage2 先对转账预览仅匹配转账分类规则，并用隐藏支出/收入侧字段分别匹配来源/目标账户规则；再对剩余预览用投资分类规则识别投资，来源账户按 parser/支付方式匹配，投资账户按交易对方优先、描述兜底匹配；最后按当前收入/支出类型限定对应分类规则和账户规则。默认分类规则和导入阶段内置兜底会把理财收益、投资支出、公交地铁等常见账单补齐到有效一级/二级分类路径；转账预览保留源金额和目标金额，供前端展示转出/转入金额。learning 建议会生成 user-scoped `recommendation_key` 并写入生命周期：默认 `yellow` 只显示推荐信号和接受/拒绝按钮，不自动改写预览字段；同一建议接受达到 3 次后变为 `green`，后续导入可自动应用分类/账户并只在前端保留拒绝按钮；green 拒绝 2 次降回 yellow，yellow 拒绝 3 次写入 suppression 后停止推荐。接受/拒绝事件落在 `import_learning_feedback_events`，生命周期状态落在 `import_learning_lifecycle`，抑制状态落在 `import_learning_suppressions`；带转账证据的 learning 建议不可改变类型，只推荐 transfer-compatible 分类和账户，冲突的收入/支出/投资 learned type 会被忽略。确定性 learning 未命中时，运行态 Weaviate 用 counterparty/description/composite 特征召回派生建议；该建议仍使用本地 recommendation key 与生命周期展示 yellow 信号，Weaviate metadata 单独不能启用 auto-apply。拒绝 pending 建议会回退到 stage2 已持久化的分类规则/账户基线，人工改字段只清除这些 actionable 建议提示而不覆盖人工值；确认、取消和失败后新建 session 都会清理当前用户的 import staging。账户识别规则已有 Rust core、SQLite/PostgreSQL schema、user-scoped repository 和 API；导入 stage2 以 `account_rules` 为账户识别权威，旧账户别名只作为规则迁移输入，不再独立写入正式预览账户字段。

### 分类与账户规则

分类识别使用 canonical `category_rules` 规则表达式；账户识别使用 `account_rules` 表和与分类规则一致的规则表达式匹配器。PostgreSQL cutover 下，分类规则列表、创建、更新、删除、重排、测试和 legacy `/api/categories/rules` 配置缓存直接读写 PostgreSQL `category_rules` / `settings`；账户规则列表、创建、更新、删除、重排、测试和别名迁移直接读写 PostgreSQL `account_rules`。账户规则按当前用户绑定到账户，支持账户角色范围、交易类型范围、字段范围、优先级、启停、正则开关、匹配计数，以及从可见账户别名幂等迁移的 source/source_key。REST API 覆盖 `GET|POST /api/account-rules/`、`PUT|DELETE /api/account-rules/{rule_id}`、`POST /api/account-rules/{rule_id}/test`、`POST /api/account-rules/reorder` 和 `POST /api/account-rules/migrate-aliases`；PostgreSQL 模式的设置包导出投影 `accounts`、`transactionCategories`、`transactionTags`、`transactionTemplates`、`scheduledTransactions`、`categoryRecognitionRules` 和 `accountRecognitionRules` 分区，恢复导入路径接收账户、分类、标签、分类规则和账户规则，暂未接管的模板、LLM/OCR 配置分区只返回跳过警告或空导出。

桌面规则中心的“规则配置”包含分类识别、账户识别和周期识别三个二级页；账户识别页复用分类规则表达式展示密度，提供账户、角色范围、交易类型范围、字段范围、优先级、启停、正则、测试、重排、删除和别名迁移控件。桌面账户编辑保留旧别名输入，并在已持久化账户上嵌入该账户过滤后的规则管理面板。移动端通过 `/account/rules` 提供 Framework7 账户规则列表与紧凑编辑/测试/迁移入口，移动账户编辑页也保留别名输入并跳转到对应账户规则。

### Matching

历史正式账单的转账候选与导入既有账单转账配对保持同一核心约束：同日、5 分钟内、金额绝对值在 1 分容差内且方向相反、来源账户不同，并排除已配对或已 suppressed 的账单。账单详情接受历史 `transfer` 候选时会保留当前账单 id，将两条收支合成为一条带来源账户、目标账户和目标金额的 `转账` 账单，并删除另一条账单；接受历史 `duplicate` 候选时会保留当前账单、合并标签并删除候选重复账单。拒绝 transfer/duplicate 会写入对应 suppression，避免同一对账单再次提示。

规则中心的配对总览展示转账配对和重复配对；旧投资配对入口不再作为总览主视图，投资相关规则仍保留在分类/规则治理链路中。

### 金额

数据库核心金额通常按元存储，部分前端/API 交互使用分。账单、账户、预算、统计或导入金额字段发生变化时，必须显式复核元/分转换。

### 预算与统计

预算按月/季/年层级同步，删除主预算或最后一个子预算时会清理派生父周期预算；PostgreSQL 模式下预算 CRUD、导出、导入、执行、预测和历史快照按同一层级、筛选、账户/标签上下文与名称更新语义执行，并把 API 元单位金额写入 `budgets.amount_cents` 与 `budget_history.*_amount_cents` 分单位列。统计链路由 Rust 读取账户、分类、汇率和资产趋势数据，并保持前端图表契约。

### 认证与备份

Rust auth runtime 校验 Bearer access token；PostgreSQL cutover 下登录、注册、邮箱验证、密码找回/重置、refresh token、token session 管理、2FA status/verify/enable/disable/recovery、Profile 读取/更新/头像、cloud settings/profile verification resend/external auth list+unlink、user-data 统计/导出/清空直接读写 `users`、`token_sessions`、`user_two_factor_recovery_codes`、`user_external_auths`、`accounts`、`categories`、`settings` 与 `business_audit_events`，Bearer 解析不再打开 SQLite session fallback，logout 使用 PostgreSQL `token_sessions` 撤销当前 token hash。legacy SQLite auth runtime 仍在显式 test-only 配置下保留 session 校验和撤销；Postgres 模式的访问控制边界是签名与过期时间有效的 JWT 加 PostgreSQL token session 管理。2FA、step-up、user-data clear、backup file 操作按当前用户和动作类型执行额外校验，并写入认证或业务审计；PostgreSQL user-data clear 使用 PostgreSQL auth event 窗口限流与业务审计，不回退到 SQLite。备份 runtime 负责本地 zip、加密备份公开名、下载、删除、恢复、cleanup、jobs 与 cloud sync，PostgreSQL authority 下记录、任务和审计元数据只写 `backup_records`、`backup_jobs`、`backup_audit_logs`，不再打开 SQLite backup_ops schema。

### LLM/OCR

LLM 临时配置保存在 Rust 进程内 user-scoped map，saved config 落库并按既有 API key 规则脱敏；saved config 还保存 provider auth profile，支持 API key、session/auth/account/Sub2API JSON、access token、refresh token、token endpoint 和刷新 headers/body/params。provider 生成保留 allowlist/SSRF 防护、响应体上限、候选截断和 rate limit，并在 access token 过期或 provider 返回 401 时尝试刷新，失败时返回前端可识别的重登状态。OCR recognition 默认 disabled，配置后可通过 Tesseract、本地 JSON OCR 或 LLM vision provider 返回结构化交易草稿；OCR 配置同样支持 model、base URL、parameters 和脱敏 auth profile。设置包普通导出脱敏 LLM/OCR 凭据，敏感分区导出需要密码并包含真实密钥。

### 前端主题

前端主题由 `src/web/src/core/theme.ts` 的统一注册表管理。桌面 Vuetify、移动端 Framework7、应用设置页与系统主题自动解析都从同一注册表读取主题名称、明暗属性、色板、移动端 CSS 变量和 meta theme-color；当前保留 `auto` / `light` / `dark` 兼容值，并为 Halloween、forest、wireframe、black、dracula、business、night、dim 等经典预设提供浅色/深色配对。桌面表格条纹、表头和主题切换按钮使用主题 token 与配对关系，避免深色主题下出现硬编码浅色条纹。金额收入/支出颜色仍由独立 amount-color 体系管理，不并入主题状态色。

## 文档入口

- [后端导航图](backend-map.md) — Rust 后端分层、请求生命周期、导入管线、repository 数据流和验证矩阵
- [总体架构](overview-architecture.md) — Rust 后端、前端、SQLite 与 REST 主链模式
- [后端模块](overview-backend.md) — HTTP/Core/DB/Parser 责任边界
- [API 路由](overview-api-routes.md) — `/api/...` route modules 和合同约束
- [导入链路](overview-import.md) — v2 三阶段导入、preview、learning、LLM/OCR
- [导入全链路验收场景](import-full-chain-scenarios.md) — parser-first、stage2、preview、confirm、PostgreSQL/Weaviate 必需运行态矩阵
- [数据库与数据流](overview-database.md) — repository、事务、user-scope 和 staging 生命周期
- [PostgreSQL migration tooling](postgres-migration.md) — SQLite dry-run、导出 bundle、导入校验、retry/rollback
- [Weaviate derived index](weaviate-derived-index.md) — 必需向量派生索引配置、bootstrap、outbox 和 rebuild
- [Matching 域](overview-matching.md) — transfer/investment/learning/recurring 配对候选
- [统计与汇率](overview-statistics.md) — 统计主链、汇率 REST 和用户数据管理
- [认证与安全](overview-auth-security.md) — 认证、2FA、token、backup、step-up 和审计
- [测试结构](overview-testing.md) — Rust、前端和治理测试基线

## 验证基线

后端业务改动以 Rust gate 为准：

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90
```

前端改动至少运行：

```powershell
Set-Location src\web
npm run lint
npm run test:coverage
npm run build
```
