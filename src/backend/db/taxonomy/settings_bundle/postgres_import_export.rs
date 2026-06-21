// 中文导读：PostgreSQL authority 设置包导入 facade，按 section importer 聚合导入编排。
// 维护重点：保持 user-scope、幂等 upsert 和引用重映射；不要回退到 non-Postgres runtime。
// 不变式：dry_run 必须 rollback；真实导入必须按用户事务提交；设置包机器金额字段必须使用显式 cents/minor units。

include!("postgres_import_export/orchestrator.rs");
include!("postgres_import_export/accounts.rs");
include!("postgres_import_export/categories.rs");
include!("postgres_import_export/tags.rs");
include!("postgres_import_export/templates.rs");
include!("postgres_import_export/category_rules.rs");
include!("postgres_import_export/account_rules.rs");
include!("postgres_import_export/helpers.rs");
#[cfg(test)]
include!("postgres_import_export/tests.rs");
