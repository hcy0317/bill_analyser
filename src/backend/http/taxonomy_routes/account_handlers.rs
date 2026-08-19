// 中文导读：账户 HTTP handler facade，聚合账户 CRUD、排序、余额同步和交易迁移/清理功能文件。
// 维护重点：handler 只编排认证、请求 DTO 和 repository/runtime 调用；SQL 与批量变更 helper 保持在对应子文件。
// 不变式：所有账户路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

include!("account_handlers/crud.rs");
include!("account_handlers/display_order.rs");
include!("account_handlers/balance_sync.rs");
include!("account_handlers/move_transactions.rs");
include!("account_handlers/clear_transactions.rs");
