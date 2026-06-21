// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

include!("preview_mutation_helpers/selection_payload.rs");
include!("preview_mutation_helpers/session_update_payload.rs");
include!("preview_mutation_helpers/patch_builder.rs");
include!("preview_mutation_helpers/update_payloads.rs");
include!("preview_mutation_helpers/decisions.rs");
include!("preview_mutation_helpers/llm_decisions.rs");
include!("preview_mutation_helpers/value_changes.rs");
include!("preview_mutation_helpers/response_projection.rs");
include!("preview_mutation_helpers/tests.rs");
