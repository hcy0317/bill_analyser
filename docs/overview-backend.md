# 后端模块

后端是 Rust workspace，按 runtime、core、database、parser 分层。

## `src/backend/http`

Axum HTTP 入口，负责路由注册、认证上下文、请求解析、multipart 上传、response envelope 和 structured error。主要 route modules：

- `auth_routes/` facade + auth route helper shards
- `bill_routes/`
- `import_routes/`
- `taxonomy_routes/` facade + taxonomy route helper shards for accounts/tags, category mutations, category formatters, settings bundle handlers, serialization, audit/rule helpers and common helpers
- `budget_routes.rs` facade + `budget_routes/`
- `statistics_routes/`
- `matching_routes.rs`
- `backup_routes/`

## `src/backend/db`

SQLite repository 层，负责 WAL/FK 连接配置、schema 初始化、事务 helper、user-scope 查询、业务表 CRUD 和导入 preview/session staging。Auth repository 由 `auth.rs` facade 聚合 `auth/` 下的 user/session/profile/cloud settings/2FA/log/row helper 模块；auth registration 由 `auth_registration.rs` facade 聚合默认 seed、注册、分类、账户与测试模块；import staging repository 由 `import_staging.rs` facade 聚合 `import_staging/` 下的 session/template/preview/decision/LLM memory/confirm/row helper 模块；matching repository 由 `matching.rs` facade 聚合 `matching/` 下的 schema/actions/candidate query/reconciliation/serialization/helper 模块；budget repository 由 `budgets.rs` facade 聚合 `budgets/` 下的 CRUD/import/execution/history/forecast/hierarchy/row helper 模块；taxonomy settings bundle repository 由 `taxonomy/settings_bundle/mod.rs` facade 聚合导入、导出、规范化和 JSON helper 分片。

## `src/backend/core`

共享业务合同与治理层，包含迁移治理清单、金额/时间/统计/分类相关公共规则、预算 period/category/history/forecast/export 合同、matching candidate/learning/investment/recurring 规则、按 LLM config/provider/prompt/response 与 OCR config/parser 分片的 `ai_ocr_llm` 合同、auth/security/ops/import pipeline 合同和 CLI bridge 测试目标。

## `src/backend/parsers`

账单解析器 crate，提供 parser registry、`RawBill` / `StandardBill`、parser tags、provider-specific CSV/XLS/XLSX/HTML-xls 解析和 golden fixture 合同。

## 约束

- Route handler 不直接承载复杂 SQL；复杂读写下沉到 repository/runtime。
- API 兼容优先通过 DTO/envelope/adapter 层处理，不在前端 store 中重复补丁。
- 业务写入保持事务原子性、user-scope 和审计 best-effort。
