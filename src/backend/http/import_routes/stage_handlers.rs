// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

include!("stage_handlers/dedup_handler.rs");
include!("stage_handlers/decision_group_materialization.rs");
include!("stage_handlers/stage2_types.rs");
include!("stage_handlers/stage2_application.rs");
include!("stage_handlers/stage2_chain.rs");
include!("stage_handlers/stage2_transfer_invariants.rs");
include!("stage_handlers/stage2_category_rules.rs");
include!("stage_handlers/stage2_learning_rules.rs");
include!("stage_handlers/stage2_recurring_accounts.rs");
include!("stage_handlers/preview_patch_projection.rs");
include!("stage_handlers/confirm_handler.rs");
include!("stage_handlers/session_handlers.rs");
include!("stage_handlers/preview_page_handlers.rs");
include!("stage_handlers/preview_selection_handler.rs");
include!("stage_handlers/preview_mutation_handlers.rs");
include!("stage_handlers/recurring_transfer_handlers.rs");
include!("stage_handlers/learning_handlers.rs");
include!("stage_handlers/tests.rs");
