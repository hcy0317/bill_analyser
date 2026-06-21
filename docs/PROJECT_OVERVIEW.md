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

导入预览后端当前以 facade + 功能子文件组织：`src/backend/db/import_staging.rs` 聚合 session、parser/source staging、preview query/predicate/selection/write/identity/confirm/materialization/row mapping/filter matching 等子文件；`src/backend/http/import_routes/stage_handlers.rs`、`preview_mutation_helpers.rs` 与 `stage_vector_recall.rs` 聚合 dedup/stage2、preview page/selection/update/reclassify、confirm、learning、vector recall 等子文件。该拆分只改变物理结构，公开函数名、REST 路由、PostgreSQL 合同与导入语义保持不变。

身份设置后端当前以 facade + 功能子文件组织：`src/backend/db/taxonomy/postgres_reads.rs` 聚合账户、分类、分类规则、账户规则、标签、模板、row mapping 与 helper 子文件；`settings_bundle/postgres_import_export.rs` 聚合 settings bundle 导入编排、账户/分类/标签/模板/规则 section importer 与金额/ref helper；`src/backend/http/taxonomy_routes/account_handlers.rs` 聚合账户 CRUD、排序、余额同步、交易迁移和清理；`account_category_formatters/accounts_and_tags.rs` 聚合账户子账户、响应格式化、请求 payload、legacy 类型分类归一化、标签和模板格式化。该拆分不改变 REST route、user-scope SQL、settings bundle 顺序、legacy 恢复兼容或 cents/minor units 金额合同。

身份设置前端当前以页面 facade + 功能文件夹组织：桌面账户、分类、标签列表页和移动账户编辑页保留页面入口，模板与样式下沉到相邻功能文件夹；账户编辑弹窗保留脚本入口并外置模板/样式；桌面分类列表的页面状态、表单、导入导出和操作回调收敛到 `useCategoryListPage.ts` composable。该拆分不改变按钮入口、导入导出控件、REST/store 调用、账户余额聚合、分类/标签编辑合同或现有视觉布局。

导入预览桌面前端当前以页面 facade + 功能子文件组织：`ImportDialog.vue` 保留导入流程编排和模板入口，`import-dialog/**` 承载导入源选择、进度条、配置匹配、preview page 查询参数和样式；`ImportTransactionCheckDataTab.vue` 保留预览表格入口和事件合同，`check-data-tab/**` 承载筛选菜单和批量动作；`checkDataMatching.ts` 与 `importPreviewIndex.ts` 作为兼容 facade 聚合 `check-data-matching/**` 和 `import-preview-index/**` 的信号视图模型、历史改写、筛选分组、查询过滤与 mapping helper。该拆分不改变 prop/emit、REST 调用、金额/身份合同或导入预览视觉布局。

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

金额在数据库、Rust core/DB/HTTP DTO、REST API、前端模型、services、stores 与测试中统一使用整数分或显式 minor units 字段。后端 JSON 使用 `*_cents` / `*_minor_units`，前端 JSON 和模型使用 `*Cents` / `*MinorUnits`；预算、账户、交易、导入预览、统计、对账、matching、交易模板、周期模板与设置包都不再用裸 `amount`、`balance`、`sourceAmount`、`destinationAmount` 表达金额。元单位只存在于用户输入/展示格式化，以及 parser、OCR、LLM、原始导入源等外部边界；进入业务 DTO 前立即归一化为整数分。

## 导入链路

导入链路由 Rust 完成 parser-first 上传、JSON parse、session/source/template/standard-row staging、dedup、转账 materialization、分类规则、recurring/learning/LLM decision、账户规则匹配、preview page、preview update/reclassify、confirm 与 cleanup。多文件 staging 与 preview row 落库使用分块批量写入；导入首屏只等待 preview row 和历史改写物化完成，decision group 证据在 preview 可操作后后台 best-effort 物化，不阻塞用户进入预览。preview 分页、筛选、排序、facets 和跨页选择由 PostgreSQL 在 `import_preview_rows` 上执行，跨页选择会保留“全部/有效/无效/需标注”子集语义，前端只保留当前页交互草稿。预览信号筛选按信号列可见 family 计算：parser 只匹配没有平台重复、转账匹配、历史改写、learning 和 LLM 可见信号的 parser 行；`signal=transfer` 只匹配 `preview_matching_feedback.transfer` 这类转账匹配反馈，不把普通 `transaction_type=转账` 或仅有 transfer-like dedup 文本的行当作转账匹配信号；其他信号筛选分别匹配对应可见 family。预览分类的 canonical `category_id` 会贯穿 draft、payload、mutation、筛选、confirm 和最终 bill 创建；前端预览以当前用户可见分类表中的 canonical id 为合法性依据，不再因为本地类型缓存不一致把已存在分类显示为非法；后端写入 preview 和 confirm 前仍校验分类、来源账户与目标账户来自当前用户 active 主数据，`0`、不存在、非当前用户、inactive、类型不匹配、转账同账户和 `__none__`/`__invalid__` sentinel 都不会作为有效身份写入 preview 或正式账单。

multipart 上传并行执行 dedicated parser 检测，每个文件必须且只能命中一个 dedicated parser 才会进入标准账单解析。parser 标准化会把 `不计收支` 等仅代表来源侧中性记账的原始类型按金额方向降为收入或支出，并把原始文本保留在 `original_type`/parser evidence 中。stage2 phase precedence 为：同批/跨批 dedup 与历史账单匹配先形成预览基底，只有结构化转账匹配和历史账单匹配能自动把预览行判定为转账；分类规则、recurring、learning、vector recall、LLM 或账户规则都不能自行把非转账行自动改成转账，但预览页人工编辑仍可手动选择“转账”。stage2 在分类和账户规则前会清理无结构化转账匹配授权的自动 `转账` 预览态，转账分类规则只在 `preview_matching_feedback.transfer` 明确给出未拒绝的 `transfer`/`transfer_cross_batch` 候选时运行；reclassify 复用同一规则清理既有未确认预览行的转账专属分类、目标账户、隐藏转账载荷和自动反馈。普通 preview update payload 不能覆盖 server-authoritative `matching_feedback`，只能通过可编辑字段保存时携带的手动编辑标记保护人工选择的 `转账` 类型，且该标记不授予转账分类规则权限。非转账分类规则按优先级在收入、支出、投资三类规则池内统一匹配，命中任一当前用户 active 分类规则后以该分类的类型和 `category_id` 为准重写预览类型；内置分类兜底仍只按当前类型做保守补齐，不作为独立投资关键词系统。recurring 与可自动应用的非转账 learning projection 可在收入、支出、投资之间继续改写类型、分类或显式账户，账户规则最后消费稳定后的预览类型、转账/投资上下文和仍为空的账户字段。进入 preview row、preview patch、reclassify 和 confirm 前会用当前用户 active 分类/账户 map 做身份校验；无效身份会被清空并写入 `preview_matching_feedback.identity_validation`，需要人工 review，confirm 在同一事务内锁定 selected preview rows、重读身份与 review 状态、创建账单并更新 session。原始 parser/payment/category 文本可作为 review evidence 展示，但不会回填 canonical 分类或账户 ID；支付方式、POS 渠道和 `/` 等占位符只保留在支付方式或描述证据列，不作为原始账户名或分类名兜底进入非法身份列表。确定性规则链未命中时，通过有并发上限的 Weaviate 召回派生 learning 建议；当前用户没有 `import_learning_features` 源行时会跳过 Weaviate recall，避免对不存在的向量源逐行发起无效网络请求；非转账行只查询 income、expense、investment 三个 scope，转账保护行只查询 transfer scope。Weaviate metadata 本身不会触发自动改写。LLM preview recommendation 会要求并解析 `suggested_type` 与 `suggested_category_id`，分类应用优先以当前用户 active 分类 ID 为准，旧式文本建议只能在 active 分类表中解析成功后写入 canonical 字段；没有结构化转账授权的 LLM 转账类型或转账分类只记录为 LLM 信号，不会自动把预览行升级为转账。

导入 staging 写入前统一规范化账单日期文本：常规日期/时间、银行 Excel 日期序列，以及“日期 + 小数日时间”会转换为标准 `YYYY-MM-DD HH:MM:SS` 文本再进入 PostgreSQL 时间字段。

账户识别以 `account_rules` 为权威；前端账户 DTO 不包含别名字段，账户规则以账户、表达式、优先级、启停状态为当前合同。`account_rules` 当前迁移后 schema 不再包含旧 `account_role_scope`、`transaction_type_scope` 与 `field_scope` 持久化列；旧 payload、query 或 settings bundle 中携带这些字段时只在 API/导入边界产生兼容 warning 并被忽略。导入运行时按稳定后的账单类型、账户角色和上下文字段包决定匹配目标；非正则账户规则只匹配规范化后的完整 token，避免“本行”误匹配“本行POS”等渠道文本。账户 API 在 DTO 边界把恢复数据或旧式数据中缺失的账户类型、账户分类归一成当前前端分类合同；缺失分类会优先保留显式值，再按账户名称、图标和旧式类型推断，不用统一现金兜底覆盖已恢复账户。

## 分类与规则中心

分类识别使用 `category_rules` 规则表达式；`category_rules` 后端核心当前以 facade + `types`、`parser`、`matching`、`terms`、`tests` 子文件组织，分别承载表达式 DTO、语法解析、文本匹配、term 转义/连接符工具和模块内合同测试。分类规则列表只暴露能投影出非空表达式的规则，避免旧恢复行或坏数据在规则中心显示为空匹配式。账户识别使用 `account_rules` 表和同一表达式匹配器模型；`account_rules` 后端核心当前以 facade + `engine`、`tests` 子文件组织，`engine` 承载候选编译、scope 归一、上下文字段准备和匹配解释，测试文件锁定拆分前行为。账户规则 API 和设置包导出不再传播旧 role/type/field scope 字段；旧 payload 或旧 bundle 中的 scope 字段会被忽略并返回兼容 warning。REST API 覆盖账户规则 list/create/update/delete/reorder/test，以及分类规则 list/create/update/delete/reorder/defaults/test。设置包按当前 PostgreSQL 主链导出并导入/upsert `accounts`、`transactionCategories`、`transactionTags`、`transactionTemplates`、`scheduledTransactions`、`categoryRecognitionRules` 与 `accountRecognitionRules`；`transactionTemplates` 和 `scheduledTransactions` 在账户、分类、标签引用重映射完成后写入 `transaction_templates`，有效导出不再以 unsupported section warning 跳过。

桌面规则中心的“规则配置”包含分类识别、账户识别和周期识别三个二级页；分类识别按一级分类聚类展示，同一分类目标的多条规则表达式在同一行内分行呈现；账户识别按账户主分类、父账户/子账户分组展示，主分类行只呈现图标和规则数量，同一账户下的多条规则表达式在同一行内分行呈现，不重复显示规则名、优先级、匹配次数等运行态元数据；移动端通过 `/account/rules` 提供同样分组后的账户规则列表与紧凑编辑/测试入口。

## Matching

正式账单的转账候选与导入配对保持同一核心约束：同日、5 分钟内、金额绝对值在 1 分容差内且方向相反、来源账户不同，并排除已配对或已 suppressed 的账单。接受候选会合并或删除对应账单并写入审计；拒绝候选会写入 suppression，避免同一对账单再次提示。

规则中心的配对总览展示转账配对和重复配对；投资相关识别规则保留在分类/规则治理链路中。

## 预算与统计

预算按月/季/年层级同步，删除主预算或最后一个子预算时会清理派生父周期预算。预算 CRUD、导出、导入、执行、预测和快照按同一层级、筛选、账户/标签上下文与名称更新语义执行，并通过 `amount_cents` / `amountCents` 写入 minor units 列。统计链路由 Rust 读取账户、分类、汇率和资产趋势数据，所有金额响应字段均使用显式 cents/minor units 字段，前端图表只在展示时格式化为元。

## 认证与备份

认证运行态校验 Bearer access token 签名、过期时间和 PostgreSQL token session。登录、注册、邮箱验证、密码重置、refresh、logout、2FA、profile、cloud settings、external auth、user-data statistics/export/clear 直接读写 PostgreSQL 表。敏感动作通过当前密码、操作密码或签名 step-up token 校验，并写入认证或业务审计。

注册请求支持可选 `defaultPackage`。缺失、`null` 或 `none` 保持旧注册行为；`standard_daily_v1` 会在注册事务内一次性写入面向中国大陆个人/家庭账单的默认账户、交易分类、分类规则和账户识别规则，并在注册响应中返回 `defaultSeed` 写入摘要。桌面端和移动端注册页都提供独立 opt-in 开关；未勾选的新用户和所有现有用户不会自动写入这套默认包。该默认包写入的数据仍走当前 taxonomy/settings 主链，可随设置包导出并导入到其他账号。

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
