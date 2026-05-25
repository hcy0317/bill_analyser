// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

use rusqlite::Connection;

use crate::DbResult;

#[tracing::instrument(level = "debug", skip_all)]
pub fn run_transaction<T>(
    connection: &mut Connection,
    operation: impl FnOnce(&rusqlite::Transaction<'_>) -> DbResult<T>,
) -> DbResult<T> {
    // 业务写入统一从这里获得 rollback-on-error 语义；best-effort 审计应在调用方
    // 明确隔离，不能混入必须原子提交的账单/账户/预算/导入写入。
    let transaction = connection.transaction()?;
    match operation(&transaction) {
        Ok(value) => {
            transaction.commit()?;
            Ok(value)
        }
        Err(error) => {
            transaction.rollback()?;
            Err(error)
        }
    }
}
