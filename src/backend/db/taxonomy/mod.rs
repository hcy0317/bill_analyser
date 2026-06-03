// 中文导读：PostgreSQL taxonomy DTO 与仓储入口。
// 维护重点：SQL 与数据行映射集中在 Postgres 仓储层，HTTP handler 不应复制查询逻辑。

pub mod account_rules;
pub mod accounts;
pub mod categories;
pub mod category_rules;
pub mod postgres_reads;
pub mod settings_bundle;
pub mod tags;
pub mod templates;
