// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

use bill_analyser_core::UserId;

use crate::{DbError, DbResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserScope {
    user_id: UserId,
}

impl UserScope {
    pub const fn new(user_id: UserId) -> Self {
        Self { user_id }
    }

    pub const fn user_id(self) -> UserId {
        self.user_id
    }

    pub fn where_clause(self, table_alias: &str) -> String {
        let alias = table_alias.trim();
        if alias.is_empty() || !is_safe_identifier(alias) {
            return "user_id = ?".to_string();
        }
        format!("{alias}.user_id = ?")
    }

    pub fn bind_value(self) -> DbResult<i64> {
        i64::try_from(self.user_id.get()).map_err(|_| {
            DbError::InvalidOperation("user_id does not fit sqlite INTEGER".to_string())
        })
    }
}

fn is_safe_identifier(value: &str) -> bool {
    value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
}
