// 中文导读：PostgreSQL category rule DTO。分类规则仓储读写在 `taxonomy::postgres_reads`。

use serde_json::{Map, Value};

pub type CategoryRuleRecord = Map<String, Value>;
