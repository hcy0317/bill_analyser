use bill_analyser_core::{UserDataStatisticsContract, UserId};
use rusqlite::{params, Connection};

use crate::{DbResult, UserScope};

pub fn get_user_data_statistics(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<UserDataStatisticsContract> {
    let user_id = UserScope::new(user_id).bind_value()?;
    Ok(UserDataStatisticsContract {
        bill_count: count_user_rows(connection, "bills", user_id)?,
        account_count: count_user_rows(connection, "accounts", user_id)?,
        category_count: count_user_rows(connection, "categories", user_id)?,
        tag_count: count_user_rows(connection, "tags", user_id)?,
        template_count: count_user_rows(connection, "bill_templates", user_id)?,
    })
}

fn count_user_rows(connection: &Connection, table_name: &str, user_id: i64) -> DbResult<i64> {
    let sql = format!("SELECT COUNT(*) FROM {table_name} WHERE user_id = ?1");
    connection
        .query_row(&sql, params![user_id], |row| row.get::<_, i64>(0))
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_id(value: u64) -> UserId {
        UserId::new(value).expect("positive user id")
    }

    #[test]
    fn user_data_statistics_are_user_scoped() -> DbResult<()> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(
            r#"
            CREATE TABLE bills (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE accounts (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE categories (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE tags (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE bill_templates (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);

            INSERT INTO bills(id, user_id) VALUES (1, 42), (2, 42), (3, 77);
            INSERT INTO accounts(id, user_id) VALUES (10, 42), (11, 77);
            INSERT INTO categories(id, user_id) VALUES (20, 42), (21, 42), (22, 77);
            INSERT INTO tags(id, user_id) VALUES (30, 42), (31, 42), (32, 42), (33, 77);
            INSERT INTO bill_templates(id, user_id) VALUES (40, 42), (41, 77);
            "#,
        )?;

        let statistics = get_user_data_statistics(&connection, user_id(42))?;

        assert_eq!(statistics.bill_count, 2);
        assert_eq!(statistics.account_count, 1);
        assert_eq!(statistics.category_count, 2);
        assert_eq!(statistics.tag_count, 3);
        assert_eq!(statistics.template_count, 1);
        Ok(())
    }
}
