    #[test]
    fn profile_and_cloud_setting_primitives_preserve_user_scope() -> DbResult<()> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                username TEXT NOT NULL,
                email TEXT NOT NULL,
                nickname TEXT,
                avatar TEXT,
                default_account_id INTEGER,
                transaction_edit_scope INTEGER DEFAULT 0,
                language TEXT DEFAULT 'zh_Hans',
                default_currency TEXT DEFAULT 'CNY',
                first_day_of_week INTEGER DEFAULT 1,
                fiscal_year_start INTEGER DEFAULT 1,
                calendar_display_type INTEGER DEFAULT 0,
                date_display_type INTEGER DEFAULT 0,
                long_date_format INTEGER DEFAULT 0,
                short_date_format INTEGER DEFAULT 0,
                long_time_format INTEGER DEFAULT 0,
                short_time_format INTEGER DEFAULT 0,
                fiscal_year_format INTEGER DEFAULT 0,
                currency_display_type INTEGER DEFAULT 0,
                numeral_system INTEGER DEFAULT 0,
                decimal_separator INTEGER DEFAULT 0,
                digit_grouping_symbol INTEGER DEFAULT 0,
                digit_grouping INTEGER DEFAULT 0,
                coordinate_display_type INTEGER DEFAULT 0,
                expense_amount_color INTEGER DEFAULT 0,
                income_amount_color INTEGER DEFAULT 0,
                cash_account_id INTEGER,
                cash_transfer_category_id INTEGER,
                import_learning_enabled INTEGER DEFAULT 1,
                investment_platform_keywords TEXT,
                investment_product_keywords TEXT,
                investment_exclude_keywords TEXT,
                email_verified INTEGER DEFAULT 0,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE user_application_cloud_settings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                setting_key TEXT NOT NULL,
                setting_value TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, setting_key)
            );
            CREATE TABLE user_external_auths (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                external_auth_category TEXT NOT NULL,
                external_auth_type TEXT NOT NULL,
                external_user_id TEXT,
                external_username TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, external_auth_type)
            );
            CREATE TABLE accounts (
                id INTEGER PRIMARY KEY,
                user_id INTEGER NOT NULL,
                name TEXT NOT NULL
            );
            CREATE TABLE categories (
                id INTEGER PRIMARY KEY,
                user_id INTEGER NOT NULL,
                main_category TEXT NOT NULL,
                sub_category TEXT NOT NULL
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
            INSERT INTO users(id, username, email, nickname, email_verified, updated_at)
            VALUES (42, 'alice', 'alice@example.test', '', 1, '2026-01-01T00:00:00');
            INSERT INTO users(id, username, email, nickname, updated_at)
            VALUES (7, 'bob', 'bob@example.test', 'Bob', '2026-01-01T00:00:00');
            INSERT INTO accounts(id, user_id, name) VALUES (100, 42, 'Cash'), (101, 7, 'Other Cash');
            INSERT INTO categories(id, user_id, main_category, sub_category)
            VALUES (200, 42, 'Transfer', ''), (201, 7, 'Other Transfer', '');
            INSERT INTO auth_logs(user_id, username, event_type, ip_address, user_agent, success, created_at)
            VALUES
                (42, 'alice', 'verification_email_resend_requested', '127.0.0.1', '', 1, '2026-01-03T00:00:00'),
                (42, 'alice', 'verification_email_resend_requested', '127.0.0.1', '', 1, '2026-01-03T00:04:00'),
                (7, 'bob', 'verification_email_resend_requested', '127.0.0.1', '', 1, '2026-01-03T00:04:00');
            INSERT INTO user_application_cloud_settings(user_id, setting_key, setting_value, created_at, updated_at)
            VALUES
                (42, 'showAmountInHomePage', 'true', '2026-01-01T00:00:00', '2026-01-01T00:00:00'),
                (42, 'autoSaveTransactionDraft', 'old', '2026-01-01T00:00:00', '2026-01-01T00:00:00'),
                (7, 'showAmountInHomePage', 'false', '2026-01-01T00:00:00', '2026-01-01T00:00:00');
            INSERT INTO user_external_auths(
                user_id, external_auth_category, external_auth_type,
                external_user_id, external_username, created_at, updated_at
            ) VALUES
                (42, 'oauth2', 'github', 'gh-42', 'alice-gh', '2026-01-02T00:00:00', '2026-01-02T00:00:00'),
                (42, 'oauth2', 'google', 'gg-42', NULL, '2026-01-03T00:00:00', '2026-01-03T00:00:00'),
                (7, 'oauth2', 'github', 'gh-7', 'bob-gh', '2026-01-04T00:00:00', '2026-01-04T00:00:00');
            "#,
        )?;

        assert!(update_auth_user_profile(
            &connection,
            user_id(42),
            &[
                AuthUserProfileUpdate::Nickname("Alice".to_string()),
                AuthUserProfileUpdate::Email("new-alice@example.test".to_string()),
                AuthUserProfileUpdate::Avatar("data:text/plain;base64,YQ==".to_string()),
                AuthUserProfileUpdate::Language("en".to_string()),
                AuthUserProfileUpdate::DefaultCurrency("USD".to_string()),
                AuthUserProfileUpdate::FirstDayOfWeek(2),
                AuthUserProfileUpdate::DefaultAccountId(Some(100)),
                AuthUserProfileUpdate::TransactionEditScope(3),
                AuthUserProfileUpdate::FiscalYearStart(4),
                AuthUserProfileUpdate::CalendarDisplayType(5),
                AuthUserProfileUpdate::DateDisplayType(6),
                AuthUserProfileUpdate::LongDateFormat(7),
                AuthUserProfileUpdate::ShortDateFormat(8),
                AuthUserProfileUpdate::LongTimeFormat(9),
                AuthUserProfileUpdate::ShortTimeFormat(10),
                AuthUserProfileUpdate::FiscalYearFormat(11),
                AuthUserProfileUpdate::CurrencyDisplayType(12),
                AuthUserProfileUpdate::NumeralSystem(13),
                AuthUserProfileUpdate::DecimalSeparator(14),
                AuthUserProfileUpdate::DigitGroupingSymbol(15),
                AuthUserProfileUpdate::DigitGrouping(16),
                AuthUserProfileUpdate::CoordinateDisplayType(17),
                AuthUserProfileUpdate::ExpenseAmountColor(18),
                AuthUserProfileUpdate::IncomeAmountColor(19),
                AuthUserProfileUpdate::CashAccountId(None),
                AuthUserProfileUpdate::CashTransferCategoryId(Some(200)),
                AuthUserProfileUpdate::ImportLearningEnabled(false),
                AuthUserProfileUpdate::InvestmentPlatformKeywords(
                    r#"["蚂蚁财富","雪球"]"#.to_string(),
                ),
                AuthUserProfileUpdate::InvestmentProductKeywords(r#"["基金"]"#.to_string()),
                AuthUserProfileUpdate::InvestmentExcludeKeywords(r#"["还款"]"#.to_string()),
            ],
            "2026-01-02T00:00:00",
        )?);
        assert!(!update_auth_user_profile(
            &connection,
            user_id(42),
            &[],
            "2026-01-02T00:00:00",
        )?);
        assert!(!update_auth_user_profile(
            &connection,
            user_id(999),
            &[AuthUserProfileUpdate::Nickname("Missing".to_string())],
            "2026-01-02T00:00:00",
        )?);
        let profile = get_auth_user_profile(&connection, user_id(42))?.expect("profile");
        assert_eq!(profile.nickname, "Alice");
        assert_eq!(profile.email, "new-alice@example.test");
        assert_eq!(profile.avatar, "data:text/plain;base64,YQ==");
        assert_eq!(profile.language, "en");
        assert_eq!(profile.default_currency, "USD");
        assert_eq!(profile.first_day_of_week, 2);
        assert_eq!(profile.default_account_id, Some(100));
        assert_eq!(profile.transaction_edit_scope, 3);
        assert_eq!(profile.fiscal_year_start, 4);
        assert_eq!(profile.calendar_display_type, 5);
        assert_eq!(profile.date_display_type, 6);
        assert_eq!(profile.long_date_format, 7);
        assert_eq!(profile.short_date_format, 8);
        assert_eq!(profile.long_time_format, 9);
        assert_eq!(profile.short_time_format, 10);
        assert_eq!(profile.fiscal_year_format, 11);
        assert_eq!(profile.currency_display_type, 12);
        assert_eq!(profile.numeral_system, 13);
        assert_eq!(profile.decimal_separator, 14);
        assert_eq!(profile.digit_grouping_symbol, 15);
        assert_eq!(profile.digit_grouping, 16);
        assert_eq!(profile.coordinate_display_type, 17);
        assert_eq!(profile.expense_amount_color, 18);
        assert_eq!(profile.income_amount_color, 19);
        assert_eq!(profile.cash_account_id, None);
        assert_eq!(profile.cash_transfer_category_id, Some(200));
        assert!(!profile.import_learning_enabled);
        assert!(!profile.email_verified);
        assert_eq!(
            profile.investment_platform_keywords.as_deref(),
            Some(r#"["蚂蚁财富","雪球"]"#)
        );
        assert_eq!(
            profile.investment_product_keywords.as_deref(),
            Some(r#"["基金"]"#)
        );
        assert_eq!(
            profile.investment_exclude_keywords.as_deref(),
            Some(r#"["还款"]"#)
        );
        assert_eq!(
            get_auth_user_profile(&connection, user_id(7))?
                .expect("other profile")
                .nickname,
            "Bob"
        );
        assert!(auth_email_exists_for_other_user(
            &connection,
            user_id(42),
            "bob@example.test"
        )?);
        assert!(!auth_email_exists_for_other_user(
            &connection,
            user_id(42),
            "new-alice@example.test"
        )?);
        assert!(auth_account_belongs_to_user(&connection, user_id(42), 100)?);
        assert!(!auth_account_belongs_to_user(
            &connection,
            user_id(42),
            101
        )?);
        assert!(auth_category_belongs_to_user(
            &connection,
            user_id(42),
            200
        )?);
        assert!(!auth_category_belongs_to_user(
            &connection,
            user_id(42),
            201
        )?);
        assert_eq!(
            count_auth_events_since(
                &connection,
                user_id(42),
                "verification_email_resend_requested",
                "2026-01-03T00:01:00"
            )?,
            1
        );
        let resend_draft = AuthLogDraft {
            user_id: Some(user_id(42)),
            username: "alice".to_string(),
            event_type: "verification_email_resend_requested".to_string(),
            ip_address: "198.51.100.13".to_string(),
            user_agent: "Mozilla".to_string(),
            success: true,
            error_message: None,
            metadata: Some(r#"{"email_present":true}"#.to_string()),
            created_at: "2026-01-03T00:05:00".to_string(),
        };
        assert!(create_auth_log_under_event_limit(
            &connection,
            user_id(42),
            "verification_email_resend_requested",
            "2026-01-03T00:01:00",
            2,
            &resend_draft,
        )?);
        assert!(!create_auth_log_under_event_limit(
            &connection,
            user_id(42),
            "verification_email_resend_requested",
            "2026-01-03T00:01:00",
            2,
            &resend_draft,
        )?);
        assert_eq!(
            count_auth_events_since(
                &connection,
                user_id(42),
                "verification_email_resend_requested",
                "2026-01-03T00:01:00"
            )?,
            2
        );
        let mut mismatched_resend_user = resend_draft.clone();
        mismatched_resend_user.user_id = Some(user_id(7));
        assert!(matches!(
            create_auth_log_under_event_limit(
                &connection,
                user_id(42),
                "verification_email_resend_requested",
                "2026-01-03T00:01:00",
                2,
                &mismatched_resend_user,
            ),
            Err(DbError::InvalidOperation(message))
                if message == "auth log user does not match event limit user"
        ));
        let mut mismatched_resend_event = resend_draft.clone();
        mismatched_resend_event.event_type = "profile_email_changed".to_string();
        assert!(matches!(
            create_auth_log_under_event_limit(
                &connection,
                user_id(42),
                "verification_email_resend_requested",
                "2026-01-03T00:01:00",
                2,
                &mismatched_resend_event,
            ),
            Err(DbError::InvalidOperation(message))
                if message == "auth log event type does not match event limit type"
        ));
        assert!(!update_auth_user_profile_with_auth_log(
            &connection,
            user_id(42),
            &[],
            "2026-01-02T00:01:00",
            &resend_draft,
        )?);
        assert!(update_auth_user_profile_with_auth_log(
            &connection,
            user_id(42),
            &[AuthUserProfileUpdate::Nickname("Alice Logged".to_string())],
            "2026-01-02T00:01:00",
            &AuthLogDraft {
                user_id: Some(user_id(42)),
                username: "alice".to_string(),
                event_type: "profile_email_changed".to_string(),
                ip_address: "127.0.0.1".to_string(),
                user_agent: "Mozilla".to_string(),
                success: true,
                error_message: None,
                metadata: Some(r#"{"email_changed":true}"#.to_string()),
                created_at: "2026-01-02T00:01:00".to_string(),
            },
        )?);
        assert_eq!(
            get_auth_user_profile(&connection, user_id(42))?
                .expect("logged profile")
                .nickname,
            "Alice Logged"
        );
        assert_eq!(
            count_auth_events_since(
                &connection,
                user_id(42),
                "profile_email_changed",
                "2026-01-02T00:00:00"
            )?,
            1
        );

        let external_auths = list_user_external_auths(&connection, user_id(42))?;
        assert_eq!(
            external_auths
                .iter()
                .map(|item| item.external_auth_type.as_str())
                .collect::<Vec<_>>(),
            vec!["google", "github"]
        );
        assert_eq!(external_auths[0].external_username, "");
        assert_eq!(
            get_user_external_auth(&connection, user_id(42), "github")?
                .expect("github auth")
                .external_username,
            "alice-gh"
        );
        assert!(get_user_external_auth(&connection, user_id(42), "missing")?.is_none());
        assert!(delete_user_external_auth(
            &connection,
            user_id(42),
            "github"
        )?);
        assert!(get_user_external_auth(&connection, user_id(42), "github")?.is_none());
        assert!(!delete_user_external_auth(
            &connection,
            user_id(42),
            "github"
        )?);
        assert_eq!(list_user_external_auths(&connection, user_id(7))?.len(), 1);

        assert!(update_application_cloud_settings(
            &connection,
            user_id(42),
            &[
                ApplicationCloudSettingDraft {
                    setting_key: "showAmountInHomePage".to_string(),
                    setting_value: "false".to_string(),
                },
                ApplicationCloudSettingDraft {
                    setting_key: "itemsCountInTransactionListPage".to_string(),
                    setting_value: "25".to_string(),
                },
            ],
            true,
            "2026-01-03T00:00:00",
        )?);
        let settings = list_application_cloud_settings(&connection, user_id(42))?;
        assert_eq!(settings.len(), 2);
        assert_eq!(settings[0].setting_key, "showAmountInHomePage");
        assert_eq!(settings[0].setting_value, "false");
        assert!(settings
            .iter()
            .all(|setting| setting.setting_key != "autoSaveTransactionDraft"));
        assert_eq!(
            list_application_cloud_settings(&connection, user_id(7))?.len(),
            1
        );

        assert!(update_application_cloud_settings(
            &connection,
            user_id(42),
            &[
                ApplicationCloudSettingDraft {
                    setting_key: "   ".to_string(),
                    setting_value: "ignored".to_string(),
                },
                ApplicationCloudSettingDraft {
                    setting_key: "autoSaveTransactionDraft".to_string(),
                    setting_value: "draft".to_string(),
                },
            ],
            false,
            "2026-01-03T00:01:00",
        )?);
        let settings = list_application_cloud_settings(&connection, user_id(42))?;
        assert_eq!(settings.len(), 3);
        assert!(settings
            .iter()
            .any(|setting| setting.setting_key == "autoSaveTransactionDraft"));

        assert!(update_application_cloud_settings(
            &connection,
            user_id(42),
            &[],
            true,
            "2026-01-03T00:02:00",
        )?);
        assert!(list_application_cloud_settings(&connection, user_id(42))?.is_empty());

        assert!(delete_application_cloud_settings(&connection, user_id(42))?);
        assert!(list_application_cloud_settings(&connection, user_id(42))?.is_empty());
        assert_eq!(
            list_application_cloud_settings(&connection, user_id(7))?.len(),
            1
        );
        Ok(())
    }

    #[test]
    fn profile_mutations_roll_back_on_log_or_cloud_setting_errors() -> DbResult<()> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                username TEXT NOT NULL,
                nickname TEXT,
                updated_at TEXT NOT NULL
            );
            INSERT INTO users(id, username, nickname, updated_at)
            VALUES (42, 'alice', 'Alice', '2026-01-01T00:00:00');
            "#,
        )?;
        let result = update_auth_user_profile_with_auth_log(
            &connection,
            user_id(42),
            &[AuthUserProfileUpdate::Nickname("Rolled Back".to_string())],
            "2026-01-02T00:00:00",
            &AuthLogDraft {
                user_id: Some(user_id(42)),
                username: "alice".to_string(),
                event_type: "profile_email_changed".to_string(),
                ip_address: "127.0.0.1".to_string(),
                user_agent: "Mozilla".to_string(),
                success: true,
                error_message: None,
                metadata: None,
                created_at: "2026-01-02T00:00:00".to_string(),
            },
        );
        assert!(result.is_err());
        assert_eq!(
            connection.query_row("SELECT nickname FROM users WHERE id = 42", [], |row| {
                row.get::<_, String>(0)
            })?,
            "Alice"
        );

        assert!(update_application_cloud_settings(
            &connection,
            user_id(42),
            &[ApplicationCloudSettingDraft {
                setting_key: "showAmountInHomePage".to_string(),
                setting_value: "true".to_string(),
            }],
            false,
            "2026-01-02T00:00:00",
        )
        .is_err());
        Ok(())
    }
