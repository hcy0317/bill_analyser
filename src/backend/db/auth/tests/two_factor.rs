    #[test]
    fn two_factor_primitives_preserve_recovery_code_scope_and_edges() -> DbResult<()> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                two_factor_enabled INTEGER DEFAULT 0,
                two_factor_secret TEXT DEFAULT '',
                updated_at TEXT NOT NULL
            );
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
            CREATE TABLE user_two_factor_recovery_codes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                code_hash TEXT NOT NULL,
                used_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            INSERT INTO users(id, two_factor_enabled, two_factor_secret, updated_at)
            VALUES (42, 0, '', '2026-01-01T00:00:00'),
                   (77, 1, 'already-enabled', '2026-01-01T00:00:00');
            "#,
        )?;

        assert!(hash_two_factor_recovery_code("ABCD-EFGH").is_some());
        assert!(hash_two_factor_recovery_code("   ").is_none());
        assert_eq!(
            replace_two_factor_recovery_codes(
                &connection,
                user_id(42),
                &["ABCD-EFGH", "ABCD-EFGH", "    ", "IJKL-MNOP"],
                "2026-01-02T00:00:00",
            )?,
            2
        );
        assert_eq!(
            count_active_two_factor_recovery_codes(&connection, user_id(42))?,
            2
        );
        assert!(consume_two_factor_recovery_code(
            &connection,
            user_id(42),
            "ABCD-EFGH",
            "2026-01-02T00:01:00",
        )?);
        assert!(!consume_two_factor_recovery_code(
            &connection,
            user_id(42),
            "ABCD-EFGH",
            "2026-01-02T00:02:00",
        )?);
        assert!(!consume_two_factor_recovery_code(
            &connection,
            user_id(42),
            "    ",
            "2026-01-02T00:02:00",
        )?);
        assert_eq!(
            count_active_two_factor_recovery_codes(&connection, user_id(42))?,
            1
        );

        let session_draft = CreateTokenSessionDraft {
            user_id: user_id(42),
            token_hash: "token".to_string(),
            refresh_token_hash: Some("refresh".to_string()),
            expires_at: "2099-01-01T00:00:00".to_string(),
            refresh_expires_at: Some("2099-01-02T00:00:00".to_string()),
            user_agent: "Mozilla".to_string(),
            ip_address: "127.0.0.1".to_string(),
            created_at: "2026-01-02T00:03:00".to_string(),
        };
        let mismatched_draft = CreateTokenSessionDraft {
            user_id: user_id(77),
            ..session_draft.clone()
        };
        assert!(matches!(
            enable_two_factor_with_recovery_codes_and_session(
                &connection,
                user_id(42),
                "secret",
                &["QRST-UVWX"],
                &mismatched_draft,
                "2026-01-02T00:03:00",
            ),
            Err(DbError::InvalidOperation(_))
        ));

        let (stored_count, session_id) = enable_two_factor_with_recovery_codes_and_session(
            &connection,
            user_id(42),
            "secret",
            &["QRST-UVWX", "QRST-UVWX", "YZAB-CDEF"],
            &session_draft,
            "2026-01-02T00:03:00",
        )?;
        assert_eq!(stored_count, 2);
        assert!(session_id > 0);
        assert_eq!(
            count_active_two_factor_recovery_codes(&connection, user_id(42))?,
            2
        );
        assert!(matches!(
            enable_two_factor_with_recovery_codes_and_session(
                &connection,
                user_id(42),
                "secret",
                &["QRST-UVWX"],
                &session_draft,
                "2026-01-02T00:04:00",
            ),
            Err(DbError::InvalidOperation(message))
                if message == "two-factor authentication is already enabled"
        ));
        assert!(matches!(
            disable_two_factor_and_clear_recovery_codes(
                &connection,
                user_id(999),
                "2026-01-02T00:05:00",
            ),
            Err(DbError::InvalidOperation(message)) if message == "user not found"
        ));
        assert_eq!(
            disable_two_factor_and_clear_recovery_codes(
                &connection,
                user_id(42),
                "2026-01-02T00:05:00",
            )?,
            2
        );
        assert_eq!(
            clear_two_factor_recovery_codes(&connection, user_id(42))?,
            0
        );

        Ok(())
    }
