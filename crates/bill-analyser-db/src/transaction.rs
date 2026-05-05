use rusqlite::Connection;

use crate::DbResult;

pub fn run_transaction<T>(
    connection: &mut Connection,
    operation: impl FnOnce(&rusqlite::Transaction<'_>) -> DbResult<T>,
) -> DbResult<T> {
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
