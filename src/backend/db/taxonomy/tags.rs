// 中文导读：PostgreSQL tag DTO。标签仓储读写在 `taxonomy::postgres_reads`。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagRecord {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub display_order: i64,
    pub hidden: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagDisplayOrder {
    pub tag_id: i64,
    pub display_order: i64,
}
