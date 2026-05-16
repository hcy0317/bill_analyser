# Bill Analyser 项目总览

Bill Analyser 是一个多来源账单导入、智能去重、自动分类、预算与统计分析全栈系统。当前运行态已经完成 Rust-only 切换：`bill_http_server` 是唯一 HTTP 服务入口，默认监听 `BILL_ANALYSER_HTTP_BIND=127.0.0.1:5000`，所有业务 API 通过 `REST /api/...` 进入 Rust Axum router；未知 `/api/...` 返回 Rust 侧结构化 404，不再透传到外部后端。

后端 Rust workspace 位于 `src/backend/*`，按功能域拆分：`src/backend/http` 提供 Axum 路由、认证上下文、上传与响应 envelope，账单、导入、认证、统计、备份、预算、分类/账户/标签/模板等大路由已拆成目录模块或 facade 聚合模块，其中 auth routes 按 public auth、profile/user-data、2FA/token、JWT/TOTP、payload、audit/response helper 分片，taxonomy route helpers 保持路由 facade 并将账户/标签、分类变更和分类格式化拆成 formatter 分片；`src/backend/db` 提供 SQLite WAL 连接、schema 初始化、事务 helper 与各业务 repository，其中 auth repository 按 user/session/profile/cloud settings/2FA/log/row helper 分片，auth registration 按默认 seed、注册、分类与账户分片，import staging 按 session/template/preview/decision/LLM memory/confirm/row helper 拆成目录模块，matching repository 按 schema/actions/candidate query/reconciliation/serialization/helper 拆成目录模块，budget repository 按 CRUD/import/execution/history/forecast/hierarchy/row helper 拆成目录模块，taxonomy settings bundle repository 按导入账户/分类/标签、模板/规则/LLM/OCR、导出格式化和 JSON helper 分片；`src/backend/core` 固定迁移治理、金额/时间/分类/统计、预算 period/category/history/forecast/export、matching candidate/learning/investment/recurring，以及按 LLM config/provider/prompt/response 与 OCR config/parser 分片的 `ai_ocr_llm` 共享业务合同；`src/backend/parsers` 提供微信、支付宝、工商银行、农业银行、建设银行、民生银行等账单解析器，dedicated parser 按来源拆在 `dedicated/WeChat.rs`、`Alipay.rs`、`ICBC.rs`、`CMBC.rs`、`ABC.rs`、`CCB.rs`，各 parser 只负责识别对应账单并映射为 `RawBill`，统一由 `post_process_raw_bills()` 标准化为 `StandardBill`。Rust 集成/契约测试集中在 `tests/backend/*` 并由各 crate manifest 显式纳入 `cargo test --workspace`；需要访问私有 helper 的 Rust 单元测试文件也集中放在 `tests/backend/**/internal/*` 后再由源码侧 `include!` / `#[path]` 挂载。前端为 Vue 3/TypeScript/Vite，桌面端使用 Vuetify，移动端使用 Framework7，统一调用 `REST /api/...`。

Rust HTTP runtime 直接接管账单导入三阶段链路、账单 CRUD/export/picture/recurring/reconciliation/category actions、账户/标签/分类/分类规则/模板/设置包、预算 CRUD/export/execution/forecast/history/snapshot/import、统计读取/Analyzer/洞察/汇率、matching/recurring/calendar/networth、auth/profile/token/2FA/step-up/user-data、LLM 配置/候选/provider 生成、OCR recognition、backup file/jobs/sync 等主链路。数据库以 SQLite 文件为默认运行库，保持 WAL、foreign keys、user-scope、事务原子性和关键审计 best-effort 写入。

前端服务层的运行态 `/api/...` 调用由 Rust `governance_manifest_snapshot().routes` 生成的 route ownership fixture 校验；`src/web/src/lib/services.ts` 保持统一 axios facade，预算 REST 查询参数、元/分适配与 response mapper 拆在 `src/web/src/lib/services/budget.ts`；`src/web/src/contracts/rustRouteOwnership.manifest.generated.json` 保存 Rust 快照映射，`rustRouteOwnership.generated.ts` 提供前端测试导入，`tests/web/contracts/frontendRustRouteContract.test.ts` 扫描 `services.ts`、Vue 与 TypeScript 里的 axios/fetch 调用，确保前端不依赖旧运行时边界或 `/api/v1/*` 路由。

## 关键业务链路

- **导入链路**：parser-first multipart 上传会并发解析多文件并保持响应顺序，且按文件保留 parser id / tags 到 template staging；JSON parse、未匹配文件列映射、session/preview/dedup/confirm、preview update/reclassify、分类规则、账户别名匹配、transfer/recurring/learning decision 和 learning promotion 都在 Rust runtime 中完成；DB staging、dedup 与 confirm 仍保持明确事务边界，dedup 可不内联返回预览列表，但仍会落库供 preview index/page 分页读取，preview matching feedback 是预览页 parser/dedup/transfer/recurring/learning 信号的统一稀疏来源，HTTP 预览响应会投影为包含 transfer/investment/learning/llm/recurring/dedup/parser/annotation/reconciliation 的完整 `matching` 视图。
- **金额边界**：数据库核心金额通常按元存储，前端/API 交互存在分字段；涉及账单、账户、预算、统计或导入字段时必须人工复核元/分转换。
- **预算层级**：预算按月/季/年层级同步，删除分类主预算或最后一个子预算时会同步清理自动派生的父周期预算，避免季度/年度空壳残留。
- **认证安全**：Rust auth runtime 校验 Bearer access token，2FA、step-up、user-data clear、backup file 操作按当前用户和动作类型执行额外校验，并写入认证或业务审计。
- **备份运维**：Rust backup runtime 负责本地 zip 备份、加密备份公开名、下载、删除、恢复、cleanup、jobs 与 cloud sync；恢复前先创建 `before_restore_*` 快照，并拒绝不安全 zip 成员。
- **LLM/OCR**：LLM 临时配置保存在 Rust 进程内 user-scoped map，saved config 落库并脱敏；provider 生成保留 allowlist/SSRF 防护、响应体上限、候选截断和 rate limit；OCR recognition 默认 disabled，配置后通过对应 provider 返回结构化识别结果。

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

## 目录

- [项目定位](overview-positioning.md) — 系统目标、核心数据源（微信/支付宝/多家银行）
- [总体架构](overview-architecture.md) — Rust 后端 workspace、前端、SQLite 与 REST 主链模式
- [后端模块](overview-backend.md) — Rust HTTP/Core/DB/Parser crate 的功能域边界
- [前端模块](overview-frontend.md) — 视图（desktop/mobile）、Pinia stores、统一 axios 服务层、设置 JSON 导入导出入口
- [API 路由与 REST 收口](overview-api-routes.md) — 账户/标签/模板/预算/账单/分类/设置包/OCR/LLM 配置各域 REST 主链
- [导入链路](overview-import.md) — v2 三阶段导入、预览校验交互、OCR 回填、学习域、LLM 推荐
- [Matching 域](overview-matching.md) — 转账/投资/learning 配对候选、generic accept/reject/clear、manual pair 管理、reconcile-history
- [统计与汇率](overview-statistics.md) — 统计主链、汇率 REST、用户数据管理
- [认证与安全](overview-auth-security.md) — 认证/2FA/token/backup/step-up/OAuth2 合同与审计
- [数据库与数据流](overview-database.md) — 主要业务表、导入三阶段临时表、导入处理顺序
- [日志与运维](overview-ops.md) — 统一日志入口、启停脚本
- [测试结构](overview-testing.md) — 测试分布、结构体量门禁、推荐验证命令
