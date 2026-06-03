// 中文导读：PostgreSQL account rule DTO。账户规则仓储读写在 `taxonomy::postgres_reads`。

use serde_json::{Map, Value};

pub type AccountRuleRecord = Map<String, Value>;
