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
                (5, 42, 'expired', '2020-01-01T00:00:00', NULL, 'expired', '127.0.0.5', 1, '2020-01-01T00:00:00', '2020-01-01T00:00:00'),
                (7, 42, 'refresh-backed', '2020-01-01T00:00:00', '2099-01-01T00:00:00', 'refresh', '127.0.0.7', 1, '2020-01-01T00:00:00', '2020-01-01T00:00:00'),
                (8, 42, 'refresh-expired', '2099-01-01T00:00:00', '2020-01-01T00:00:00', 'refresh expired', '127.0.0.8', 1, '2020-01-01T00:00:00', '2020-01-01T00:00:00');
            "#,
        )?;

        assert_eq!(
            cleanup_expired_sessions(&connection, "2026-01-01T00:00:00")?,
            2
        );
        let sessions = list_user_sessions(&connection, user_id(42))?;
        assert_eq!(
            sessions
                .iter()
                .map(|session| session.id)
                .collect::<Vec<_>>(),
            vec![2, 1, 7]
        );

        assert!(invalidate_session_by_id(&connection, 2, user_id(42))?);
        assert!(!invalidate_session_by_id(&connection, 4, user_id(42))?);
        assert_eq!(
            list_user_sessions(&connection, user_id(42))?
                .iter()
                .map(|session| session.id)
                .collect::<Vec<_>>(),
            vec![1, 7]
        );

        connection.execute(
            "INSERT INTO sessions(id, user_id, token_hash, expires_at, is_active, created_at) VALUES (6, 42, 'new', '2099-01-01T00:00:00', 1, '2026-01-06T00:00:00')",
            [],
        )?;
        assert_eq!(
            invalidate_other_user_sessions(&connection, user_id(42), 1)?,
            2
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
        connection.execute(
            "INSERT INTO sessions(id, user_id, token_hash, expires_at, is_active, created_at) VALUES (10, 42, 'logout-token', '2099-01-01T00:00:00', 1, '2026-01-08T00:00:00')",
            [],
        )?;
        let logout_session = get_active_logout_session_by_token_hash(&connection, "logout-token")?
            .expect("logout session");
        assert_eq!(logout_session.id, 10);
        assert_eq!(logout_session.user_id, user_id(42));
        assert_eq!(logout_session.username, "alice");
        assert!(invalidate_session_by_token_hash(
            &connection,
            "logout-token"
        )?);
        assert!(get_active_logout_session_by_token_hash(&connection, "logout-token")?.is_none());
        assert!(!invalidate_session_by_token_hash(
            &connection,
            "logout-token"
        )?);

        connection.execute(
            "INSERT INTO sessions(id, user_id, token_hash, refresh_token_hash, expires_at, refresh_expires_at, is_active, created_at) VALUES (11, 42, 'refresh-old-access', 'refresh-old', '2099-01-01T00:00:00', '2099-01-02T00:00:00', 1, '2026-01-09T00:00:00')",
            [],
        )?;
        let rotated_session_id = rotate_refresh_token_session(
            &connection,
            11,
            "refresh-old",
            &CreateTokenSessionDraft {
                user_id: user_id(42),
                token_hash: "refresh-new-access".to_string(),
                refresh_token_hash: Some("refresh-new".to_string()),
                expires_at: "2099-01-03T00:00:00".to_string(),
                refresh_expires_at: Some("2099-01-04T00:00:00".to_string()),
                user_agent: "Refresh".to_string(),
                ip_address: "127.0.0.10".to_string(),
                created_at: "2026-01-09T00:01:00".to_string(),
            },
        )?
        .expect("refresh rotation session id");
        assert!(rotated_session_id > 11);
        assert_eq!(
            connection.query_row("SELECT is_active, refresh_token_hash FROM sessions WHERE id = 11", [], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
            })?,
            (0, None)
        );
        assert!(rotate_refresh_token_session(
            &connection,
            11,
            "refresh-old",
            &CreateTokenSessionDraft {
                user_id: user_id(42),
                token_hash: "refresh-retry-access".to_string(),
                refresh_token_hash: Some("refresh-retry".to_string()),
                expires_at: "2099-01-03T00:00:00".to_string(),
                refresh_expires_at: Some("2099-01-04T00:00:00".to_string()),
                user_agent: "Refresh".to_string(),
                ip_address: "127.0.0.10".to_string(),
                created_at: "2026-01-09T00:02:00".to_string(),
            },
        )?
        .is_none());
        Ok(())
    }
