// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

include!("runtime_audit_helpers/postgres_helpers.rs");
include!("runtime_audit_helpers/payloads.rs");
include!("runtime_audit_helpers/response_helpers.rs");
include!("../../../../tests/backend/http/internal/auth_runtime_audit_helpers.rs");
