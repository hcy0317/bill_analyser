// 中文导读：PostgreSQL account DTO。账户仓储读写在 `taxonomy::postgres_reads`。

use serde_json::{Map, Value};

pub type AccountRecord = Map<String, Value>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountDisplayOrder {
    pub account_id: i64,
    pub display_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountTransactionsMoveResult {
    pub success: bool,
    pub message: String,
    pub moved_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountTransactionsClearResult {
    pub success: bool,
    pub message: String,
    pub deleted_count: i64,
}
