#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_helpers_preserve_postgres_json_edges() {
        let metadata = json!({
            "name_string": " Alice ",
            "name_number": 42,
            "name_bool": true,
            "empty": " ",
            "int_number": 7,
            "int_string": "8",
            "int_bool": true,
            "optional_empty": "",
            "bool_true": "yes",
            "bool_false": 0,
            "bool_invalid": "maybe"
        });

        assert_eq!(
            metadata_string(&metadata, &["missing", "name_string"], "fallback"),
            "Alice"
        );
        assert_eq!(
            metadata_string(&metadata, &["name_number"], "fallback"),
            "42"
        );
        assert_eq!(
            metadata_string(&metadata, &["name_bool"], "fallback"),
            "true"
        );
        assert_eq!(
            metadata_string(&metadata, &["empty"], "fallback"),
            "fallback"
        );
        assert_eq!(
            metadata_optional_string(&metadata, &["name_string"]),
            Some("Alice".to_string())
        );
        assert_eq!(metadata_optional_string(&metadata, &["missing"]), None);
        assert_eq!(metadata_i64(&metadata, &["int_number"], 1), 7);
        assert_eq!(metadata_i64(&metadata, &["int_string"], 1), 8);
        assert_eq!(metadata_i64(&metadata, &["int_bool"], 1), 1);
        assert_eq!(metadata_i64(&metadata, &["missing"], 9), 9);
        assert_eq!(metadata_optional_i64(&metadata, &["int_string"]), Some(8));
        assert_eq!(metadata_optional_i64(&metadata, &["optional_empty"]), None);
        assert!(metadata_bool(&metadata, &["bool_true"], false));
        assert!(!metadata_bool(&metadata, &["bool_false"], true));
        assert!(metadata_bool(&metadata, &["bool_invalid"], true));
    }

    #[test]
    fn metadata_mutation_and_json_helpers_cover_non_object_edges() {
        let mut metadata = Value::Null;
        metadata_set_string(&mut metadata, "last_login_ip", "127.0.0.1");
        metadata_set_i64(&mut metadata, "failed_login_attempts", 3);
        metadata_set_bool(&mut metadata, "email_verified", false);
        metadata_set_optional_i64(&mut metadata, "default_account_id", Some(42));
        metadata_set_optional_i64(&mut metadata, "cash_account_id", None);

        assert_eq!(metadata["last_login_ip"], "127.0.0.1");
        assert_eq!(metadata["failed_login_attempts"], 3);
        assert_eq!(metadata["email_verified"], false);
        assert_eq!(metadata["default_account_id"], 42);
        assert!(metadata.get("cash_account_id").is_none());
        assert_eq!(
            parse_auth_metadata(Some(r#"{"reason":"failed"}"#))["reason"],
            "failed"
        );
        assert_eq!(parse_auth_metadata(Some("not-json")), Value::Null);
        assert_eq!(parse_auth_metadata(None), Value::Null);
        assert_eq!(
            json_value_to_setting_string(&Value::String("enabled".to_string())),
            "enabled"
        );
        assert_eq!(json_value_to_setting_string(&Value::Null), "");
        assert_eq!(
            json_value_to_setting_string(&json!({"enabled": true})),
            "{\"enabled\":true}"
        );
    }

    #[test]
    fn profile_update_application_covers_all_postgres_metadata_variants() {
        let mut email = "old@example.test".to_string();
        let current_email = email.clone();
        let mut display_name = "Old".to_string();
        let mut metadata = json!({ "email_verified": true });
        let updates = [
            AuthUserProfileUpdate::Nickname("New".to_string()),
            AuthUserProfileUpdate::Email("new@example.test".to_string()),
            AuthUserProfileUpdate::Avatar("data:image/png;base64,avatar".to_string()),
            AuthUserProfileUpdate::Language("en".to_string()),
            AuthUserProfileUpdate::DefaultCurrency("USD".to_string()),
            AuthUserProfileUpdate::FirstDayOfWeek(0),
            AuthUserProfileUpdate::DefaultAccountId(Some(10)),
            AuthUserProfileUpdate::TransactionEditScope(2),
            AuthUserProfileUpdate::FiscalYearStart(4),
            AuthUserProfileUpdate::CalendarDisplayType(1),
            AuthUserProfileUpdate::DateDisplayType(2),
            AuthUserProfileUpdate::LongDateFormat(3),
            AuthUserProfileUpdate::ShortDateFormat(4),
            AuthUserProfileUpdate::LongTimeFormat(5),
            AuthUserProfileUpdate::ShortTimeFormat(6),
            AuthUserProfileUpdate::FiscalYearFormat(7),
            AuthUserProfileUpdate::CurrencyDisplayType(8),
            AuthUserProfileUpdate::NumeralSystem(9),
            AuthUserProfileUpdate::DecimalSeparator(10),
            AuthUserProfileUpdate::DigitGroupingSymbol(11),
            AuthUserProfileUpdate::DigitGrouping(12),
            AuthUserProfileUpdate::CoordinateDisplayType(13),
            AuthUserProfileUpdate::ExpenseAmountColor(14),
            AuthUserProfileUpdate::IncomeAmountColor(15),
            AuthUserProfileUpdate::CashAccountId(Some(16)),
            AuthUserProfileUpdate::CashTransferCategoryId(Some(17)),
            AuthUserProfileUpdate::ImportLearningEnabled(false),
            AuthUserProfileUpdate::InvestmentPlatformKeywords("[\"ETF\"]".to_string()),
            AuthUserProfileUpdate::InvestmentProductKeywords("[\"Fund\"]".to_string()),
            AuthUserProfileUpdate::InvestmentExcludeKeywords("[\"Exclude\"]".to_string()),
        ];

        for update in &updates {
            apply_postgres_profile_update(
                update,
                &mut email,
                &current_email,
                &mut display_name,
                &mut metadata,
            );
        }

        assert_eq!(email, "new@example.test");
        assert_eq!(display_name, "New");
        assert_eq!(metadata["nickname"], "New");
        assert_eq!(metadata["email_verified"], false);
        assert_eq!(metadata["avatar"], "data:image/png;base64,avatar");
        assert_eq!(metadata["language"], "en");
        assert_eq!(metadata["default_currency"], "USD");
        assert_eq!(metadata["first_day_of_week"], 0);
        assert_eq!(metadata["default_account_id"], 10);
        assert_eq!(metadata["transaction_edit_scope"], 2);
        assert_eq!(metadata["fiscal_year_start"], 4);
        assert_eq!(metadata["calendar_display_type"], 1);
        assert_eq!(metadata["date_display_type"], 2);
        assert_eq!(metadata["long_date_format"], 3);
        assert_eq!(metadata["short_date_format"], 4);
        assert_eq!(metadata["long_time_format"], 5);
        assert_eq!(metadata["short_time_format"], 6);
        assert_eq!(metadata["fiscal_year_format"], 7);
        assert_eq!(metadata["currency_display_type"], 8);
        assert_eq!(metadata["numeral_system"], 9);
        assert_eq!(metadata["decimal_separator"], 10);
        assert_eq!(metadata["digit_grouping_symbol"], 11);
        assert_eq!(metadata["digit_grouping"], 12);
        assert_eq!(metadata["coordinate_display_type"], 13);
        assert_eq!(metadata["expense_amount_color"], 14);
        assert_eq!(metadata["income_amount_color"], 15);
        assert_eq!(metadata["cash_account_id"], 16);
        assert_eq!(metadata["cash_transfer_category_id"], 17);
        assert_eq!(metadata["import_learning_enabled"], false);
        assert_eq!(metadata["investment_platform_keywords"], "[\"ETF\"]");
        assert_eq!(metadata["investment_product_keywords"], "[\"Fund\"]");
        assert_eq!(metadata["investment_exclude_keywords"], "[\"Exclude\"]");

        for update in [
            AuthUserProfileUpdate::DefaultAccountId(None),
            AuthUserProfileUpdate::CashAccountId(None),
            AuthUserProfileUpdate::CashTransferCategoryId(None),
        ] {
            apply_postgres_profile_update(
                &update,
                &mut email,
                &current_email,
                &mut display_name,
                &mut metadata,
            );
        }
        assert!(metadata.get("default_account_id").is_none());
        assert!(metadata.get("cash_account_id").is_none());
        assert!(metadata.get("cash_transfer_category_id").is_none());

        assert!(matches!(
            postgres_auth_error(sqlx::Error::RowNotFound),
            DbError::InvalidOperation(message) if message.contains("postgres auth error")
        ));
    }

    #[tokio::test]
    async fn empty_profile_update_short_circuits_before_postgres_io() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
            .expect("lazy postgres pool");

        let changed = update_postgres_auth_user_profile(
            &pool,
            UserId::new(1).expect("user id"),
            &[],
            "2026-01-01T00:00:00Z",
        )
        .await
        .expect("empty updates do not touch postgres");

        assert!(!changed);
    }

    #[test]
    fn postgres_user_id_conversion_rejects_invalid_bigint_boundaries() {
        assert!(user_id_from_i64(-1).is_err());
        assert!(user_id_from_i64(0).is_err());
        assert_eq!(user_id_from_i64(42).unwrap().get(), 42);
        assert!(user_id_i64(UserId::new(u64::MAX).unwrap()).is_err());
    }
}
