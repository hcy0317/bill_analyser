#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_user_creates_defaults_and_logs_atomically() -> DbResult<()> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE,
                email TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                nickname TEXT,
                default_account_id INTEGER,
                language TEXT DEFAULT 'zh_Hans',
                default_currency TEXT DEFAULT 'CNY',
                first_day_of_week INTEGER DEFAULT 1,
                cash_account_id INTEGER,
                is_active INTEGER DEFAULT 1,
                email_verified INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE categories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                type INTEGER DEFAULT 1,
                main_category TEXT NOT NULL,
                sub_category TEXT NOT NULL,
                description TEXT,
                priority INTEGER DEFAULT 0,
                keywords TEXT,
                hidden INTEGER DEFAULT 0,
                icon TEXT,
                color TEXT,
                created_at TEXT NOT NULL,
                UNIQUE(user_id, main_category, sub_category)
            );
            CREATE TABLE category_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                category_id INTEGER NOT NULL,
                name TEXT NOT NULL DEFAULT '',
                priority INTEGER NOT NULL DEFAULT 100,
                rule_expression TEXT NOT NULL,
                regex_enabled INTEGER DEFAULT 0,
                enabled INTEGER DEFAULT 1,
                applied_count INTEGER DEFAULT 0,
                last_applied_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                type INTEGER NOT NULL,
                category INTEGER,
                currency TEXT DEFAULT 'CNY',
                icon TEXT,
                color TEXT,
                balance REAL DEFAULT 0,
                initial_balance REAL DEFAULT 0,
                hidden INTEGER DEFAULT 0,
                display_order INTEGER DEFAULT 0,
                comment TEXT,
                parent_id INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
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
            "#,
        )?;

        let result = create_registered_user_with_defaults(
            &connection,
            &RegisterUserDraft {
                username: "alice".to_string(),
                email: "alice@example.test".to_string(),
                password_hash: "bcrypt-hash".to_string(),
                nickname: "Alice".to_string(),
                language: "zh_Hans".to_string(),
                default_currency: "CNY".to_string(),
                first_day_of_week: 1,
                email_verified: false,
                created_at: "2026-05-09T00:00:00".to_string(),
            },
            &[RegisterPresetCategory {
                name: "自定义".to_string(),
                type_code: 3,
                icon: "custom".to_string(),
                color: "123456".to_string(),
                sub_categories: vec![RegisterPresetSubCategory {
                    name: "子类".to_string(),
                    icon: "".to_string(),
                    color: "".to_string(),
                }],
            }],
            &AuthLogDraft {
                user_id: None,
                username: "alice".to_string(),
                event_type: "register_success".to_string(),
                ip_address: "127.0.0.1".to_string(),
                user_agent: "Mozilla".to_string(),
                success: true,
                error_message: None,
                metadata: None,
                created_at: "2026-05-09T00:00:00".to_string(),
            },
        )?;

        assert_eq!(result.user_id, 1);
        assert!(result.preset_categories_saved);
        assert!(result.preset_accounts_saved);
        assert_eq!(result.default_account_id, Some(1));
        assert_eq!(result.cash_account_id, Some(1));
        assert!(result.default_seed.categories_created > 80);
        assert_eq!(
            result.default_seed.rules_created,
            DEFAULT_DAILY_CATEGORY_RULES.len() as i64
        );
        assert!(auth_username_exists(&connection, "alice")?);
        assert!(auth_email_exists(&connection, "alice@example.test")?);
        assert_eq!(
            connection.query_row("SELECT email_verified FROM users WHERE id = 1", [], |row| {
                row.get::<_, i64>(0)
            })?,
            0
        );
        assert_eq!(
            connection.query_row(
                "SELECT COUNT(*) FROM categories WHERE user_id = 1 AND main_category = '自定义'",
                [],
                |row| row.get::<_, i64>(0),
            )?,
            2
        );
        assert_eq!(
            connection.query_row(
                "SELECT type FROM categories WHERE user_id = 1 AND main_category = '账户互转' AND sub_category = ''",
                [],
                |row| row.get::<_, i64>(0),
            )?,
            TRANSFER
        );
        assert_eq!(
            connection.query_row(
                "SELECT COUNT(*) FROM category_rules
                 WHERE user_id = 1
                   AND name IN (
                       'default:投资收益/理财收益',
                       'default:金融保险/投资支出',
                       'default:交通出行/公交地铁'
                   )",
                [],
                |row| row.get::<_, i64>(0),
            )?,
            3
        );
        assert_eq!(
            connection.query_row(
                "SELECT name FROM accounts WHERE id = 1",
                [],
                |row| row.get::<_, String>(0),
            )?,
            "现金"
        );
        assert_eq!(
            connection.query_row(
                "SELECT event_type FROM auth_logs WHERE user_id = 1",
                [],
                |row| { row.get::<_, String>(0) }
            )?,
            "register_success"
        );
        let duplicate_result = create_registered_user_with_defaults(
            &connection,
            &RegisterUserDraft {
                username: "alice".to_string(),
                email: "alice@example.test".to_string(),
                password_hash: "other-hash".to_string(),
                nickname: "Alice Duplicate".to_string(),
                language: "zh_Hans".to_string(),
                default_currency: "CNY".to_string(),
                first_day_of_week: 1,
                email_verified: false,
                created_at: "2026-05-09T00:01:00".to_string(),
            },
            &[],
            &AuthLogDraft {
                user_id: None,
                username: "alice".to_string(),
                event_type: "register_success".to_string(),
                ip_address: "127.0.0.1".to_string(),
                user_agent: "Mozilla".to_string(),
                success: true,
                error_message: None,
                metadata: None,
                created_at: "2026-05-09T00:01:00".to_string(),
            },
        );
        assert!(duplicate_result.is_err());
        assert_eq!(
            connection.query_row("SELECT COUNT(*) FROM users", [], |row| {
                row.get::<_, i64>(0)
            })?,
            1
        );
        Ok(())
    }
}
