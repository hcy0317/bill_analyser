// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

use crate::{DbResult, SqliteDbPath};

#[derive(Debug, Clone)]
pub struct SqliteConnectionConfig {
    pub path: SqliteDbPath,
    pub create_if_missing: bool,
    pub busy_timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PragmaSnapshot {
    pub journal_mode: String,
    pub foreign_keys: bool,
    pub synchronous: i64,
}

pub struct SqliteRuntime {
    connection: Connection,
}

impl SqliteRuntime {
    pub fn open(config: SqliteConnectionConfig) -> DbResult<Self> {
        if config.create_if_missing {
            if let Some(parent) = config.path.as_path().parent() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let flags = if config.create_if_missing {
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        };
        let connection = Connection::open_with_flags(config.path.as_path(), flags)?;
        connection.busy_timeout(config.busy_timeout)?;
        apply_runtime_pragmas(&connection)?;
        Ok(Self { connection })
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.connection
    }

    pub fn pragma_snapshot(&self) -> DbResult<PragmaSnapshot> {
        let journal_mode: String = self
            .connection
            .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))?;
        let foreign_keys: i64 = self
            .connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))?;
        let synchronous: i64 = self
            .connection
            .query_row("PRAGMA synchronous", [], |row| row.get::<_, i64>(0))?;

        Ok(PragmaSnapshot {
            journal_mode: journal_mode.to_lowercase(),
            foreign_keys: foreign_keys == 1,
            synchronous,
        })
    }
}

fn apply_runtime_pragmas(connection: &Connection) -> DbResult<()> {
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA cache_size=10000;
         PRAGMA temp_store=MEMORY;",
    )?;
    Ok(())
}
