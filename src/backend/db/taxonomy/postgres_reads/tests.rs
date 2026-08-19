#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;
    use std::{env, error::Error};

    #[test]
    fn category_names_from_postgres_path_preserve_current_main_and_sub_fields() {
        assert_eq!(
            category_names_from_postgres_path(Some("餐饮/午餐"), "午餐"),
            ("餐饮".to_string(), "午餐".to_string())
        );
        assert_eq!(
            category_names_from_postgres_path(Some("餐饮"), "餐饮"),
            ("餐饮".to_string(), String::new())
        );
        assert_eq!(
            category_names_from_postgres_path(Some(""), "未分类"),
            ("未分类".to_string(), String::new())
        );
    }

    #[test]
    fn metadata_bool_accepts_current_json_encodings() {
        assert_eq!(
            metadata_bool(&serde_json::json!({"hidden": true}), "hidden"),
            Some(true)
        );
        assert_eq!(
            metadata_bool(&serde_json::json!({"hidden": 0}), "hidden"),
            Some(false)
        );
        assert_eq!(
            metadata_bool(&serde_json::json!({"hidden": "false"}), "hidden"),
            Some(false)
        );
    }

    #[test]
    fn postgres_template_type_normalization_matches_frontend_contract() {
        assert_eq!(normalize_template_transaction_type(Some("收入")), 2);
        assert_eq!(normalize_template_transaction_type(Some("transfer")), 4);
        assert_eq!(normalize_template_transaction_type(Some("投资")), 5);
        assert_eq!(normalize_template_transaction_type(None), 3);
    }

    #[test]
    fn explicit_minor_units_require_strict_integer_values() {
        assert_eq!(
            strict_minor_units_value(&serde_json::json!(1999), "sourceAmountCents")
                .expect("integer cents"),
            1999
        );
        assert_eq!(
            strict_minor_units_value(&serde_json::json!("1999"), "sourceAmountCents")
                .expect("integer string cents"),
            1999
        );
        for value in [
            serde_json::json!(18.49),
            serde_json::json!("18.5"),
            serde_json::json!(true),
            serde_json::json!({}),
        ] {
            assert!(strict_minor_units_value(&value, "sourceAmountCents").is_err());
        }
    }

    #[test]
    fn postgres_rule_helpers_normalize_current_expression_payloads() {
        assert_eq!(
            rule_expression_string(&serde_json::json!({
                "operator": "contains_any",
                "values": ["招商", "余额+宝", 123, true, null]
            })),
            r"OR={招商,余额\+宝,123,true}"
        );
        assert_eq!(
            rule_expression_string(&serde_json::json!({
                "expression": "OR={午餐}",
                "regex_enabled": "true"
            })),
            "OR={午餐}"
        );
        assert!(rule_expression_regex_enabled(&serde_json::json!({
            "regex_enabled": "true"
        })));
        assert!(payload_bool_value(&serde_json::json!("on")).expect("truthy string"));
        assert!(!payload_bool_value(&serde_json::json!(0)).expect("false numeric"));
        assert!(payload_bool_value(&serde_json::json!(1.5)).is_err());
        assert_eq!(
            required_postgres_i64(&serde_json::json!("42"), "field").expect("int text"),
            42
        );
        assert!(required_postgres_rule_expression(Some(&serde_json::json!(" "))).is_err());
    }

    #[test]
    fn category_rule_display_guard_rejects_legacy_only_empty_expression_projection() {
        assert_eq!(
            rule_expression_string(&serde_json::json!({
                "legacy_expression": "OR={旧规则}"
            })),
            ""
        );

        let mut legacy_only_record = CategoryRuleRecord::new();
        legacy_only_record.insert("rule_expression".to_string(), serde_json::json!(" "));
        assert!(!category_rule_has_displayable_expression(
            &legacy_only_record
        ));

        let mut current_record = CategoryRuleRecord::new();
        current_record.insert(
            "rule_expression".to_string(),
            serde_json::json!("OR={午餐}"),
        );
        assert!(category_rule_has_displayable_expression(&current_record));
        assert!(category_rule_result_has_displayable_expression(&Ok(
            current_record
        )));
        assert!(category_rule_result_has_displayable_expression(&Err(
            DbError::InvalidOperation("row projection failed".to_string())
        )));
    }

    #[test]
    fn account_rule_candidate_ignores_deprecated_scope_columns() {
        let mut record = AccountRuleRecord::new();
        record.insert("id".to_string(), serde_json::json!(7));
        record.insert("account_id".to_string(), serde_json::json!(42));
        record.insert(
            "account_role_scope".to_string(),
            serde_json::json!("wallet"),
        );
        record.insert(
            "transaction_type_scope".to_string(),
            serde_json::json!("refund"),
        );
        record.insert("field_scope".to_string(), serde_json::json!(["bad"]));
        record.insert(
            "rule_expression".to_string(),
            serde_json::json!("OR={工资卡}"),
        );
        record.insert("regex_enabled".to_string(), serde_json::json!(false));
        record.insert("enabled".to_string(), serde_json::json!(true));
        record.insert("priority".to_string(), serde_json::json!(5));

        let candidate = account_rule_candidate_from_record(record).expect("candidate");

        assert_eq!(candidate.rule_id, 7);
        assert_eq!(candidate.account_id, 42);
        assert_eq!(candidate.rule_expression, "OR={工资卡}");
        assert!(!candidate.regex_enabled);
        assert!(candidate.enabled);
        assert_eq!(candidate.priority, 5);
    }

    #[test]
    fn account_metadata_and_template_values_use_explicit_cents_fields() {
        assert_eq!(
            metadata_initial_balance_cents(&json!({"initial_balance": "12.34"}), 100),
            100
        );
        assert_eq!(
            metadata_initial_balance_cents(&json!({"initial_balance_cents": "1234"}), 100),
            1234
        );

        let metadata = account_metadata_from_payload(
            None,
            &json!({
                "initialBalanceCents": 4567,
                "balance": 99.99,
                "comment": "开户"
            }),
        )
        .expect("metadata cents");
        assert_eq!(metadata["initial_balance_cents"], json!(4567));
        assert!(metadata.get("initial_balance").is_none());
        assert!(
            account_metadata_from_payload(None, &json!({"initialBalanceCents": "1.5"})).is_err()
        );
        assert!(
            account_metadata_from_payload(None, &json!({"initial_balance_cents": true})).is_err()
        );

        let values = template_values_from_payload(
            &json!({
                "name": "月租",
                "sourceAmountCents": 12345,
                "destinationAmountCents": 54321,
                "hideAmount": true
            }),
            2,
            7,
            None,
        )
        .expect("template cents");
        assert_eq!(values.source_amount_minor_units, 12345);
        assert_eq!(values.destination_amount_minor_units, 54321);
        assert!(values.hide_amount);
        assert!(
            template_values_from_payload(&json!({"sourceAmountCents": 12.34}), 1, 8, None).is_err()
        );
        assert!(template_values_from_payload(
            &json!({"destinationAmountCents": "12.34"}),
            1,
            8,
            None
        )
        .is_err());
        assert!(
            template_values_from_payload(&json!({"sourceAmountCents": true}), 1, 8, None).is_err()
        );

        let existing = TemplateRecord::from_iter([
            ("sourceAmountCents".to_string(), json!(111)),
            ("destinationAmountCents".to_string(), json!("222")),
        ]);
        let fallback =
            template_values_from_payload(&json!({"name": "月租"}), 1, 8, Some(&existing))
                .expect("existing strict cents");
        assert_eq!(fallback.source_amount_minor_units, 111);
        assert_eq!(fallback.destination_amount_minor_units, 222);
    }

    #[test]
    fn rules_overview_recurring_payload_uses_explicit_cents() {
        let payload = rules_overview_recurring_rule_payload(
            7,
            "月租".to_string(),
            12345,
            None,
            Some(2),
            true,
            Some("2026-06-12".to_string()),
        );

        assert_eq!(payload["amountCents"], json!(12345));
        assert!(payload.get("amount").is_none());
        assert_eq!(payload["frequency"], json!("2"));
    }

    #[tokio::test]
    async fn account_and_template_row_projectors_emit_explicit_cents_when_database_available(
    ) -> Result<(), Box<dyn Error>> {
        let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
            return Ok(());
        };
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&postgres_url)
            .await?;

        let account_row = sqlx::query(
            r##"
            SELECT
                1::BIGINT AS id,
                2::BIGINT AS user_id,
                '招商银行'::TEXT AS name,
                'bank'::TEXT AS account_type,
                'CNY'::TEXT AS currency,
                123456::BIGINT AS balance_cents,
                true AS is_active,
                3::INT AS display_order,
                '{"category":2,"initial_balance_cents":654321,"icon":"card","color":"#fff"}'::jsonb AS metadata,
                now() AS created_at,
                now() AS updated_at,
                '招商卡'::TEXT AS payment_method
            "##,
        )
        .fetch_one(&pool)
        .await?;
        let account = account_from_postgres_row(account_row).expect("account record");
        assert_eq!(account.get("balanceCents"), Some(&json!(123456)));
        assert_eq!(account.get("initialBalanceCents"), Some(&json!(654321)));
        assert!(account.get("balance").is_none());

        let template_row = sqlx::query(
            r#"
            SELECT
                7::BIGINT AS id,
                2::INT AS template_type,
                '月租'::TEXT AS name,
                NULL::TEXT AS description,
                'expense'::TEXT AS transaction_type,
                '9'::TEXT AS category_id,
                '11'::TEXT AS source_account_id,
                '0'::TEXT AS destination_account_id,
                12345::BIGINT AS source_amount_minor_units,
                0::BIGINT AS destination_amount_minor_units,
                false AS hide_amount,
                '[]'::jsonb AS tag_ids,
                NULL::TEXT AS comment,
                NULL::TEXT AS scheduled_frequency,
                NULL::INT AS scheduled_frequency_type,
                NULL::TEXT AS scheduled_start_date,
                NULL::TEXT AS scheduled_end_date,
                NULL::TEXT AS scheduled_next_date,
                true AS enabled,
                false AS auto_create,
                1::INT AS display_order,
                false AS hidden,
                480::INT AS utc_offset,
                now() AS created_at,
                now() AS updated_at
            "#,
        )
        .fetch_one(&pool)
        .await?;
        let template = template_from_postgres_row(template_row).expect("template record");
        assert_eq!(template.get("sourceAmountCents"), Some(&json!(12345)));
        assert_eq!(template.get("destinationAmountCents"), Some(&json!(0)));
        assert!(template.get("sourceAmount").is_none());

        Ok(())
    }
}
