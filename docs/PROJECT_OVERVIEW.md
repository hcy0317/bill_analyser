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
- `src/backend/db`：SQLite WAL/FK 连接、schema 初始化、事务 helper、user-scope repository、导入 staging、auth、budget、matching、taxonomy/settings 等仓储模块；同时提供 PostgreSQL 迁移骨架、权威库 schema foundation、SQLite/Postgres repository runtime provider。Postgres 目前作为可配置的迁移目标和 lazy repository runtime，不静默接管业务 repository。
- `src/backend/parsers`：微信、支付宝、工商银行、农业银行、建设银行、民生银行等 dedicated parser，以及 `RawBill` 到 `StandardBill` 的统一标准化。

详细的代码阅读路径、请求生命周期、导入管线、repository 数据流和验证矩阵见 `docs/backend-map.md`。

## 数据库运行配置

默认运行态仍使用 SQLite，`BILL_ANALYSER_SQLITE_DB_PATH` 指向当前业务库，`BILL_ANALYSER_SQLITE_LEGACY_PATH` 用于迁移期显式标识 legacy SQLite 来源；未配置 legacy path 时沿用当前 SQLite 路径。PostgreSQL 切换基座通过 `BILL_ANALYSER_DATABASE_BACKEND`、`BILL_ANALYSER_POSTGRES_URL`、`BILL_ANALYSER_MIGRATION_MODE` 和 `BILL_ANALYSER_REQUIRE_POSTGRES_AFTER_CUTOVER` 暴露，默认 `database_backend=sqlite`、`migration_mode=disabled`，因此不影响本地 SQLite 启动链路。

`/api/health` 的 details 暴露 `database_backend`、`route_repository_backend`、`postgres_configured`、`postgres_url_redacted`、`migration_mode`、`migration_status`、`weaviate_status` 与 `require_postgres_after_cutover`。健康信息只显示脱敏 Postgres URL；`route_repository_backend=sqlite_legacy` 表示 route 仍通过统一边界打开 SQLite 仓储，`postgres_pending_repositories` 表示配置选择了 Postgres 但具体业务仓储尚未接管，route helper 会显式拒绝而不是静默回退。迁移和 Weaviate 状态在当前阶段是占位观测字段，后续切片会接入实际迁移 runner 和向量同步 outbox。

PostgreSQL 迁移工具入口是 `bill_sqlite_to_postgres_migrate`，支持 `dry-run`、`export`、`import-check` 和事务性 `import`。它从 SQLite 读取 legacy 表，校验必需列，生成确定性 checksum，并把账户别名转换成目标 `account_rules` payload；`import` 通过 SQLx 写入 PostgreSQL、记录 `migration_audit_events`、修正 identity 序列，不改写 SQLite，也不把业务 repository 切到 Postgres。操作步骤见 [PostgreSQL migration tooling](postgres-migration.md)。

## 关键业务链路

### 导入

导入链路由 Rust runtime 完成 parser-first 上传、JSON parse、session/template staging、dedup、账户别名匹配、分类规则、transfer/recurring/learning/LLM decision、preview page、preview update/reclassify、confirm 和 cleanup。混合来源上传按文件保留 parser id / tags；Check Data 首屏读取分页 preview page；默认分类规则和导入阶段内置兜底会把理财收益、投资支出、公交地铁等常见账单补齐到有效一级/二级分类路径；转账预览保留源金额和目标金额，供前端展示转出/转入金额。预览中的 transfer、learning 和 LLM 建议只在 pending 状态展示接受/拒绝动作，learning `auto_applied` / `auto-applied` 投影为已应用状态并只保留撤销入口；拒绝 pending 建议会回退到 stage2 已持久化的分类规则/账户基线，人工改字段只清除这些 actionable 建议提示而不覆盖人工值；确认、取消和失败后新建 session 都会清理当前用户的 import staging。账户识别规则已经有 Rust core、SQLite/PostgreSQL schema、user-scoped repository 和 API，但导入 stage2 当前只 shadow 读取规则候选，正式账户匹配仍保持既有别名链路，避免在 UI/导入切换切片完成前改变导入结果。

### 分类与账户规则

分类识别使用 canonical `category_rules` 规则表达式；账户识别使用 `account_rules` 表和与分类规则一致的规则表达式匹配器。账户规则按当前用户绑定到账户，支持账户角色范围、交易类型范围、字段范围、优先级、启停、正则开关、匹配计数，以及从可见账户别名幂等迁移的 source/source_key。REST API 覆盖 `GET|POST /api/account-rules/`、`PUT|DELETE /api/account-rules/{rule_id}`、`POST /api/account-rules/{rule_id}/test`、`POST /api/account-rules/reorder` 和 `POST /api/account-rules/migrate-aliases`；设置包导入导出包含 `accountRecognitionRules` 分区。

### Matching

历史正式账单的转账候选与导入既有账单转账配对保持同一核心约束：同日、5 分钟内、金额绝对值在 1 分容差内且方向相反、来源账户不同，并排除已配对或已 suppressed 的账单。账单详情接受历史 `transfer` 候选时会保留当前账单 id，将两条收支合成为一条带来源账户、目标账户和目标金额的 `转账` 账单，并删除另一条账单；接受历史 `duplicate` 候选时会保留当前账单、合并标签并删除候选重复账单。拒绝 transfer/duplicate 会写入对应 suppression，避免同一对账单再次提示。

规则中心的配对总览展示转账配对和重复配对；旧投资配对入口不再作为总览主视图，投资相关规则仍保留在分类/规则治理链路中。

### 金额

数据库核心金额通常按元存储，部分前端/API 交互使用分。账单、账户、预算、统计或导入金额字段发生变化时，必须显式复核元/分转换。

### 预算与统计

预算按月/季/年层级同步，删除主预算或最后一个子预算时会清理派生父周期预算。统计链路由 Rust 读取账户、分类、汇率和资产趋势数据，并保持前端图表契约。

### 认证与备份

Rust auth runtime 校验 Bearer access token，2FA、step-up、user-data clear、backup file 操作按当前用户和动作类型执行额外校验，并写入认证或业务审计。备份 runtime 负责本地 zip、加密备份公开名、下载、删除、恢复、cleanup、jobs 与 cloud sync。

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
- [数据库与数据流](overview-database.md) — repository、事务、user-scope 和 staging 生命周期
- [PostgreSQL migration tooling](postgres-migration.md) — SQLite dry-run、导出 bundle、导入校验、retry/rollback
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
