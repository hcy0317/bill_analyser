    #[test]
    fn login_failure_primitives_cover_lockout_and_reset_edges() -> DbResult<()> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                failed_login_attempts INTEGER,
                locked_until TEXT,
                last_login_at TEXT,
                last_login_ip TEXT
            );
            INSERT INTO users(id, failed_login_attempts, locked_until) VALUES
                (1, 0, NULL),
                (2, 2, NULL),
                (3, 8, '2020-01-01T00:00:00'),
                (4, 4, '2026-01-01T00:00:00');
            "#,
        )?;

        assert_eq!(
            increment_failed_login(&connection, user_id(999), 3, "2026-01-02T00:00:00", None,)?,
            None
        );
        assert_eq!(
            increment_failed_login(&connection, user_id(1), 3, "2026-01-02T00:00:00", None)?,
            Some(LoginFailureUpdate {
                failed_attempts: 1,
                locked: false
            })
        );
        assert_eq!(
            connection.query_row(
                "SELECT failed_login_attempts, locked_until FROM users WHERE id = 1",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
            )?,
            (1, None)
        );

        assert_eq!(
            increment_failed_login(&connection, user_id(2), 3, "2026-01-02T00:00:00", None)?,
            Some(LoginFailureUpdate {
                failed_attempts: 3,
                locked: true
            })
        );
        assert_eq!(
            connection.query_row(
                "SELECT failed_login_attempts, locked_until FROM users WHERE id = 2",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
            )?,
            (3, Some("2026-01-02T00:00:00".to_string()))
        );

        assert_eq!(
            increment_failed_login(
                &connection,
                user_id(3),
                3,
                "2026-01-02T00:00:00",
                Some("2020-01-01T00:00:00"),
            )?,
            Some(LoginFailureUpdate {
                failed_attempts: 1,
                locked: false
            })
        );
        assert_eq!(
            connection.query_row(
                "SELECT failed_login_attempts, locked_until FROM users WHERE id = 3",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
            )?,
            (1, None)
        );

        assert!(clear_expired_login_lock(&connection, user_id(4))?);
        assert_eq!(
            connection.query_row(
                "SELECT failed_login_attempts, locked_until FROM users WHERE id = 4",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
            )?,
            (0, None)
        );

        assert!(update_user_last_login(
            &connection,
            user_id(1),
            "2026-01-03T00:00:00",
            "127.0.0.1",
        )?);
        assert_eq!(
            connection.query_row(
                "SELECT last_login_at, last_login_ip, failed_login_attempts, locked_until FROM users WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                }
            )?,
            (
                Some("2026-01-03T00:00:00".to_string()),
                Some("127.0.0.1".to_string()),
                0,
                None
            )
        );
        Ok(())
    }
