// 中文导读：PostgreSQL template DTO。模板仓储读写在 `taxonomy::postgres_reads`。

use serde_json::{Map, Value};

pub type TemplateRecord = Map<String, Value>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateDisplayOrder {
    pub template_id: i64,
    pub display_order: i64,
}
