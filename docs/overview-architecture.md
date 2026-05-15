# 总体架构

Bill Analyser 当前是 Rust-only 后端 + Vue 前端 + SQLite 本地数据的单体应用。

## 运行入口

- 后端：`src/backend/http/bin/bill_http_server.rs`
- 默认监听：`BILL_ANALYSER_HTTP_BIND=127.0.0.1:5000`
- 前端：`src/web`，开发态默认 `http://127.0.0.1:8081`
- API 主链：`REST /api/...`

未知 `/api/...` 请求由 Rust router 返回结构化 404，不再透传到外部后端。

## Rust workspace

- `bill-analyser-http`：Axum router、认证上下文、multipart 上传、响应 envelope、domain route modules。
- `bill-analyser-db`：SQLite 连接 guard、schema 初始化、事务 helper、repository 与 user-scope 数据访问。
- `bill-analyser-core`：共享业务合同、迁移治理、金额/时间/分类/统计等领域规则。
- `bill-analyser-parsers`：账单解析器 registry、`RawBill` / `StandardBill`、parser tags 与银行/平台 parser。

## 数据流

前端通过统一服务层访问 `/api/...`；HTTP route 解析鉴权和请求 DTO 后调用 Rust domain runtime；domain runtime 通过 repository 层读写 SQLite；响应保持前端既有 `success/data` 或 `success/result` envelope 兼容。

## 关键约束

- REST 主链不重新引入 `/api/v1/*`。
- 金额字段必须显式复核元/分边界。
- 数据库写入保持事务原子性和当前用户作用域。
- 启动脚本只启动 Rust HTTP server 和前端 dev server。
