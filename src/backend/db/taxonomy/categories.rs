// 中文导读：PostgreSQL category DTO。分类仓储读写在 `taxonomy::postgres_reads`。

use serde::Serialize;
use serde_json::{Map, Value};

pub type CategoryRecord = Map<String, Value>;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct CategoryEnsureSummary {
    pub created: i64,
    pub skipped: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CategoryStatistic {
    pub main_category: String,
    pub sub_category: String,
    pub count: i64,
    pub total_amount: f64,
}
