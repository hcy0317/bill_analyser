// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

include!("llm/preview_request.rs");
include!("llm/analyze_request.rs");
include!("llm/rule_synthesis_request.rs");
#[path = "llm/provider_protocol.rs"]
mod llm_provider_protocol;
use llm_provider_protocol::{
    llm_api_protocol_attempts, llm_provider_http_request, llm_provider_runtime_response,
    llm_status_allows_protocol_fallback,
};
include!("llm/provider_runtime.rs");
include!("llm/preview_helpers.rs");
include!("llm/rule_synthesis_helpers.rs");
include!("llm/review_memory_config.rs");
include!("llm/candidate_handlers.rs");
