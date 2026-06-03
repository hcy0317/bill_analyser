// 中文导读：PostgreSQL repository 层错误类型，负责统一仓储错误边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与缓存只有在注释明确时才能作为 best-effort。

use thiserror::Error;

pub type DbResult<T> = Result<T, DbError>;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("unsafe database path: {0}")]
    UnsafePath(String),
    #[error("invalid database operation: {0}")]
    InvalidOperation(String),
    #[error("postgres error: {0}")]
    Postgres(#[from] sqlx::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
