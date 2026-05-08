use bill_analyser_core::UserId;
use rusqlite::{params, Connection};

use crate::{DbError, DbResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSessionRow {
    pub id: i64,
    pub user_agent: String,
    pub ip_address: String,
    pub expires_at: String,
    pub last_activity_at: String,
    pub created_at: String,
}

pub fn cleanup_expired_sessions(connection: &Connection, now: &str) -> DbResult<usize> {
    Ok(connection.execute(
        r#"
        DELETE FROM sessions
        WHERE expires_at < ?1
           OR (refresh_expires_at IS NOT NULL AND refresh_expires_at < ?1)
        "#,
        [now],
    )?)
}

pub fn list_user_sessions(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<Vec<TokenSessionRow>> {
    let user_id = user_id_sql(user_id)?;
    let mut statement = connection.prepare(
        r#"
        SELECT id, user_agent, ip_address, expires_at, last_activity_at, created_at
        FROM sessions
        WHERE user_id = ?1 AND is_active = 1
        ORDER BY created_at DESC
        "#,
    )?;
    let rows = statement.query_map([user_id], |row| {
        Ok(TokenSessionRow {
            id: row.get(0)?,
            user_agent: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            ip_address: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            expires_at: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            last_activity_at: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            created_at: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

pub fn invalidate_session_by_id(
    connection: &Connection,
    session_id: i64,
    user_id: UserId,
) -> DbResult<bool> {
    let user_id = user_id_sql(user_id)?;
    let changed = connection.execute(
        "UPDATE sessions SET is_active = 0 WHERE id = ?1 AND user_id = ?2",
        params![session_id, user_id],
    )?;
    Ok(changed > 0)
}

pub fn invalidate_other_user_sessions(
    connection: &Connection,
    user_id: UserId,
    current_session_id: i64,
) -> DbResult<usize> {
    let user_id = user_id_sql(user_id)?;
    Ok(connection.execute(
        "UPDATE sessions SET is_active = 0 WHERE user_id = ?1 AND id != ?2 AND is_active = 1",
        params![user_id, current_session_id],
    )?)
}

fn user_id_sql(user_id: UserId) -> DbResult<i64> {
    i64::try_from(user_id.get()).map_err(|_| {
        DbError::InvalidOperation("user id is outside SQLite INTEGER range".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_id(value: u64) -> UserId {
        UserId::new(value).expect("positive user id")
    }

    #[test]
    fn token_session_primitives_match_python_session_scope() -> DbResult<()> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(
            r#"
            CREATE TABLE sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                token_hash TEXT NOT NULL,
                refresh_token_hash TEXT,
                expires_at TEXT NOT NULL,
                refresh_expires_at TEXT,
                user_agent TEXT,
                ip_address TEXT,
                is_active INTEGER NOT NULL DEFAULT 1,
                last_activity_at TEXT,
                created_at TEXT
            );
            INSERT INTO sessions (
                id, user_id, token_hash, expires_at, refresh_expires_at,
                user_agent, ip_address, is_active, last_activity_at, created_at
            ) VALUES
                (1, 42, 'current', '2099-01-01T00:00:00', NULL, 'Mozilla Chrome', '127.0.0.1', 1, '2026-01-02T00:00:00', '2026-01-01T00:00:00'),
                (2, 42, 'api', '2099-01-01T00:00:00', NULL, 'Bill Analyser API Token', '127.0.0.2', 1, '2026-01-03T00:00:00', '2026-01-03T00:00:00'),
                (3, 42, 'inactive', '2099-01-01T00:00:00', NULL, 'inactive', '127.0.0.3', 0, '2026-01-04T00:00:00', '2026-01-04T00:00:00'),
                (4, 7, 'other-user', '2099-01-01T00:00:00', NULL, 'other', '127.0.0.4', 1, '2026-01-05T00:00:00', '2026-01-05T00:00:00'),
                (5, 42, 'expired', '2020-01-01T00:00:00', NULL, 'expired', '127.0.0.5', 1, '2020-01-01T00:00:00', '2020-01-01T00:00:00');
            "#,
        )?;

        assert_eq!(
            cleanup_expired_sessions(&connection, "2026-01-01T00:00:00")?,
            1
        );
        let sessions = list_user_sessions(&connection, user_id(42))?;
        assert_eq!(
            sessions
                .iter()
                .map(|session| session.id)
                .collect::<Vec<_>>(),
            vec![2, 1]
        );

        assert!(invalidate_session_by_id(&connection, 2, user_id(42))?);
        assert!(!invalidate_session_by_id(&connection, 4, user_id(42))?);
        assert_eq!(
            list_user_sessions(&connection, user_id(42))?
                .iter()
                .map(|session| session.id)
                .collect::<Vec<_>>(),
            vec![1]
        );

        connection.execute(
            "INSERT INTO sessions(id, user_id, token_hash, expires_at, is_active, created_at) VALUES (6, 42, 'new', '2099-01-01T00:00:00', 1, '2026-01-06T00:00:00')",
            [],
        )?;
        assert_eq!(
            invalidate_other_user_sessions(&connection, user_id(42), 1)?,
            1
        );
        assert_eq!(list_user_sessions(&connection, user_id(42))?.len(), 1);
        assert_eq!(list_user_sessions(&connection, user_id(7))?.len(), 1);
        Ok(())
    }
}
