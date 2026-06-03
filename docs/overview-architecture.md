# 总体架构

Bill Analyser 当前主链是 Vue 3 前端、Rust Axum HTTP 后端、PostgreSQL 权威数据层和必需 Weaviate 派生向量索引。

请求链路：前端服务层调用 `/api/...`，Axum route 校验认证和 DTO，调用 core/db 层读写 PostgreSQL；导入 learning recall 在确定性规则链之后调用 Weaviate。

运行态边界：

- `bill_http_server` 是唯一 HTTP 服务入口。
- PostgreSQL 是唯一业务数据库。
- Weaviate 是必需服务，不是可选降级路径。
- 前端只依赖当前 REST DTO 与当前 route ownership fixture。
- 业务金额字段在进入或离开 DTO 边界时显式处理元/分转换。

开发时优先保持 route facade 薄、domain contract 明确、repository user-scoped、前端服务层与后端 DTO 同步。
