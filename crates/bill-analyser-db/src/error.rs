use thiserror::Error;

pub type DbResult<T> = Result<T, DbError>;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("unsafe database path: {0}")]
    UnsafePath(String),
    #[error("invalid database operation: {0}")]
    InvalidOperation(String),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
