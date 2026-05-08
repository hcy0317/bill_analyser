use bill_analyser_core::UserId;
use rusqlite::{params, Connection, OptionalExtension};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthTokenUserRow {
    pub id: UserId,
    pub username: String,
    pub password_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTokenSessionDraft {
    pub user_id: UserId,
    pub token_hash: String,
    pub refresh_token_hash: Option<String>,
    pub expires_at: String,
    pub refresh_expires_at: Option<String>,
    pub user_agent: String,
    pub ip_address: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthLogDraft {
    pub user_id: Option<UserId>,
    pub username: String,
    pub event_type: String,
    pub ip_address: String,
    pub user_agent: String,
    pub success: bool,
    pub error_message: Option<String>,
    pub metadata: Option<String>,
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

pub fn get_auth_token_user(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<Option<AuthTokenUserRow>> {
    let user_id_sql = user_id_sql(user_id)?;
    connection
        .query_row(
            r#"
            SELECT id, username, password_hash
            FROM users
            WHERE id = ?1
            "#,
            [user_id_sql],
            |row| {
                let raw_id: i64 = row.get(0)?;
                let raw_id = u64::try_from(raw_id)
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, raw_id))?;
                let id = UserId::new(raw_id)
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, raw_id as i64))?;
                Ok(AuthTokenUserRow {
                    id,
                    username: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    password_hash: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                })
            },
        )
        .optional()
        .map_err(DbError::from)
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

pub fn create_token_session(
    connection: &Connection,
    draft: &CreateTokenSessionDraft,
) -> DbResult<i64> {
    let user_id = user_id_sql(draft.user_id)?;
    connection.execute(
        r#"
        INSERT INTO sessions (
            user_id, token_hash, refresh_token_hash, expires_at, refresh_expires_at,
            user_agent, ip_address, is_active, last_activity_at, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, NULL, ?8)
        "#,
        params![
            user_id,
            draft.token_hash,
            draft.refresh_token_hash,
            draft.expires_at,
            draft.refresh_expires_at,
            draft.user_agent,
            draft.ip_address,
            draft.created_at,
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

pub fn create_auth_log(connection: &Connection, draft: &AuthLogDraft) -> DbResult<i64> {
    let user_id = draft.user_id.map(user_id_sql).transpose()?;
    connection.execute(
        r#"
        INSERT INTO auth_logs (
            user_id, username, event_type, ip_address, user_agent,
            success, error_message, metadata, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
        params![
            user_id,
            draft.username,
            draft.event_type,
            draft.ip_address,
            draft.user_agent,
            if draft.success { 1 } else { 0 },
            draft.error_message,
            draft.metadata,
            draft.created_at,
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

pub fn count_recent_token_password_failures(
    connection: &Connection,
    user_id: UserId,
    since: &str,
) -> DbResult<i64> {
    let user_id = user_id_sql(user_id)?;
    connection
        .query_row(
            r#"
            SELECT COUNT(*)
            FROM auth_logs
            WHERE user_id = ?1
              AND success = 0
              AND event_type IN ('api_token_generate_failed', 'mcp_token_generate_failed')
              AND created_at >= ?2
            "#,
            params![user_id, since],
            |row| row.get(0),
        )
        .map_err(DbError::from)
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
            CREATE TABLE auth_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER,
                username TEXT,
                event_type TEXT NOT NULL,
                ip_address TEXT,
                user_agent TEXT,
                success INTEGER NOT NULL,
                error_message TEXT,
                metadata TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                username TEXT NOT NULL,
                password_hash TEXT NOT NULL
            );
            INSERT INTO users(id, username, password_hash) VALUES (42, 'alice', 'hash');
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

        let user = get_auth_token_user(&connection, user_id(42))?.expect("token user");
        assert_eq!(user.username, "alice");
        assert_eq!(user.password_hash, "hash");
        let session_id = create_token_session(
            &connection,
            &CreateTokenSessionDraft {
                user_id: user_id(42),
                token_hash: "issued-token".to_string(),
                refresh_token_hash: None,
                expires_at: "2099-01-01T00:00:00".to_string(),
                refresh_expires_at: None,
                user_agent: "Bill Analyser API Token".to_string(),
                ip_address: "127.0.0.9".to_string(),
                created_at: "2026-01-07T00:00:00".to_string(),
            },
        )?;
        assert!(session_id > 0);
        assert_eq!(
            create_auth_log(
                &connection,
                &AuthLogDraft {
                    user_id: Some(user_id(42)),
                    username: "alice".to_string(),
                    event_type: "api_token_generate_success".to_string(),
                    ip_address: "127.0.0.9".to_string(),
                    user_agent: "Bill Analyser API Token".to_string(),
                    success: true,
                    error_message: None,
                    metadata: Some(format!(r#"{{"session_id":{session_id}}}"#)),
                    created_at: "2026-01-07T00:00:00".to_string(),
                },
            )?,
            1
        );
        assert_eq!(
            count_recent_token_password_failures(&connection, user_id(42), "2026-01-06T23:59:00")?,
            0
        );
        for created_at in [
            "2026-01-07T00:01:00",
            "2026-01-07T00:02:00",
            "2026-01-07T00:03:00",
        ] {
            create_auth_log(
                &connection,
                &AuthLogDraft {
                    user_id: Some(user_id(42)),
                    username: "alice".to_string(),
                    event_type: "api_token_generate_failed".to_string(),
                    ip_address: "127.0.0.9".to_string(),
                    user_agent: "Mozilla".to_string(),
                    success: false,
                    error_message: Some("Invalid password".to_string()),
                    metadata: None,
                    created_at: created_at.to_string(),
                },
            )?;
        }
        create_auth_log(
            &connection,
            &AuthLogDraft {
                user_id: Some(user_id(42)),
                username: "alice".to_string(),
                event_type: "api_token_generate_failed".to_string(),
                ip_address: "127.0.0.8".to_string(),
                user_agent: "Mozilla".to_string(),
                success: false,
                error_message: Some("Invalid password".to_string()),
                metadata: None,
                created_at: "2026-01-07T00:04:00".to_string(),
            },
        )?;
        assert_eq!(
            count_recent_token_password_failures(&connection, user_id(42), "2026-01-07T00:00:00")?,
            4
        );
        Ok(())
    }
}
