# 后端模块

后端是 Rust workspace，按 runtime、core、database、parser 分层。

## `crates/bill-analyser-http`

Axum HTTP 入口，负责路由注册、认证上下文、请求解析、multipart 上传、response envelope 和 structured error。主要 route modules：

- `auth_routes.rs`
- `bill_routes.rs`
- `import_routes.rs`
- `taxonomy_routes.rs`
- `budget_routes.rs`
- `statistics_routes.rs`
- `matching_routes.rs`
- `backup_routes.rs`

## `crates/bill-analyser-db`

SQLite repository 层，负责 WAL/FK 连接配置、schema 初始化、事务 helper、user-scope 查询、业务表 CRUD 和导入 preview/session staging。

## `crates/bill-analyser-core`

共享业务合同与治理层，包含迁移治理清单、金额/时间/统计/分类相关公共规则、auth/security/ops/import pipeline 合同和 CLI bridge 测试目标。

## `crates/bill-analyser-parsers`

账单解析器 crate，提供 parser registry、`RawBill` / `StandardBill`、parser tags、provider-specific CSV/XLS/XLSX/HTML-xls 解析和 golden fixture 合同。

## 约束

- Route handler 不直接承载复杂 SQL；复杂读写下沉到 repository/runtime。
- API 兼容优先通过 DTO/envelope/adapter 层处理，不在前端 store 中重复补丁。
- 业务写入保持事务原子性、user-scope 和审计 best-effort。
