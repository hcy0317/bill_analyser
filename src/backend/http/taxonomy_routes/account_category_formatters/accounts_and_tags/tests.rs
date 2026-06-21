#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_account_to_frontend_normalizes_missing_legacy_category_and_type() {
        let mut account = Map::new();
        account.insert("id".to_string(), Value::Number(Number::from(17000000044_i64)));
        account.insert("name".to_string(), Value::String("工商银行".to_string()));
        account.insert("type".to_string(), Value::Number(Number::from(0)));
        account.insert("category".to_string(), Value::Number(Number::from(0)));
        account.insert("hidden".to_string(), Value::Bool(false));

        let frontend = backend_account_to_frontend(account);

        assert_eq!(frontend.get("type").and_then(Value::as_i64), Some(1));
        assert_eq!(frontend.get("category").and_then(Value::as_i64), Some(2));
        assert_eq!(frontend.get("hidden").and_then(Value::as_bool), Some(false));
        assert_eq!(frontend.get("visible").and_then(Value::as_bool), Some(true));
    }

    #[test]
    fn backend_account_to_frontend_preserves_known_category_and_multi_type() {
        let mut account = Map::new();
        account.insert("id".to_string(), Value::Number(Number::from(93)));
        account.insert("name".to_string(), Value::String("支付宝（三方主账户）".to_string()));
        account.insert("type".to_string(), Value::Number(Number::from(2)));
        account.insert("category".to_string(), Value::Number(Number::from(4)));
        account.insert("balance_cents".to_string(), Value::Number(Number::from(1234)));
        account.insert("balance".to_string(), Value::Number(Number::from(99)));

        let frontend = backend_account_to_frontend(account);

        assert_eq!(frontend.get("type").and_then(Value::as_i64), Some(2));
        assert_eq!(frontend.get("category").and_then(Value::as_i64), Some(4));
        assert_eq!(frontend.get("balanceCents").and_then(Value::as_i64), Some(1234));
        assert!(frontend.get("balance").is_none());
    }

    #[test]
    fn backend_account_to_frontend_maps_legacy_text_category_hints() {
        let mut account = Map::new();
        account.insert("id".to_string(), Value::Number(Number::from(38)));
        account.insert("name".to_string(), Value::String("农业银行信用卡".to_string()));
        account.insert("type".to_string(), Value::String("credit_card".to_string()));
        account.insert("category".to_string(), Value::Null);

        let frontend = backend_account_to_frontend(account);

        assert_eq!(frontend.get("type").and_then(Value::as_i64), Some(1));
        assert_eq!(frontend.get("category").and_then(Value::as_i64), Some(3));
    }

    #[test]
    fn backend_account_to_frontend_maps_legacy_text_category_hint_variants() {
        let cases = [
            ("investment", 7),
            ("loan", 5),
            ("receivable", 6),
            ("bank", 2),
            ("saving", 8),
            ("virtual_account", 4),
        ];

        for (account_type, expected_category) in cases {
            let mut account = Map::new();
            account.insert("id".to_string(), Value::Number(Number::from(1)));
            account.insert("name".to_string(), Value::String(account_type.to_string()));
            account.insert("type".to_string(), Value::String(account_type.to_string()));

            let frontend = backend_account_to_frontend(account);

            assert_eq!(
                frontend.get("category").and_then(Value::as_i64),
                Some(expected_category)
            );
        }
    }

    #[test]
    fn backend_account_to_frontend_normalizes_legacy_multi_type_string() {
        let mut account = Map::new();
        account.insert("id".to_string(), Value::Number(Number::from(93)));
        account.insert("name".to_string(), Value::String("支付宝".to_string()));
        account.insert(
            "type".to_string(),
            Value::String("multiple_sub_accounts".to_string()),
        );

        let frontend = backend_account_to_frontend(account);

        assert_eq!(frontend.get("type").and_then(Value::as_i64), Some(2));
        assert_eq!(frontend.get("category").and_then(Value::as_i64), Some(4));
    }

    #[test]
    fn backend_account_to_frontend_infers_recovered_account_category_distribution() {
        let cases = [
            ("农业银行", 1, "", 2),
            ("农业银行信用卡", 1, "110", 3),
            ("邮储银行信用卡", 1, "110", 3),
            ("浦发银行信用卡", 1, "110", 3),
            ("民生银行", 1, "100", 2),
            ("建设银行", 1, "100", 2),
            ("工商银行", 0, "100", 2),
            ("微信", 1, "8302", 4),
            ("中信建投证券", 1, "801", 7),
            ("支付宝（三方主账户）", 2, "8300", 4),
            ("支付宝（投资主账户）", 2, "8300", 7),
            ("花呗", 1, "8300", 4),
            ("活期资产", 1, "8300", 7),
            ("稳健理财", 1, "8300", 7),
            ("进阶理财", 1, "8300", 7),
            ("垫付款", 1, "700", 6),
            ("借账单", 1, "700", 6),
            ("现金", 1, "1", 1),
        ];

        for (name, account_type, icon, expected_category) in cases {
            let mut account = Map::new();
            account.insert("id".to_string(), Value::Number(Number::from(1)));
            account.insert("name".to_string(), Value::String(name.to_string()));
            account.insert(
                "type".to_string(),
                Value::Number(Number::from(account_type)),
            );
            account.insert("category".to_string(), Value::Null);
            account.insert("icon".to_string(), Value::String(icon.to_string()));

            let frontend = backend_account_to_frontend(account);

            assert_eq!(
                frontend.get("category").and_then(Value::as_i64),
                Some(expected_category),
                "{name} should map to category {expected_category}"
            );
        }
    }

    #[test]
    fn frontend_account_to_backend_uses_explicit_cents_fields() {
        let backend = frontend_account_to_backend(&json!({
            "name": "招商银行",
            "balanceCents": 123456,
            "visible": false,
            "displayOrder": 3
        }))
        .expect("backend payload");

        assert_eq!(backend.get("balance_cents"), Some(&json!(123456)));
        assert_eq!(backend.get("initial_balance_cents"), Some(&json!(123456)));
        assert_eq!(backend.get("hidden"), Some(&json!(true)));
        assert!(backend.get("balance").is_none());

        let fallback = frontend_account_to_backend(&json!({
            "name": "期初资产",
            "initialBalanceCents": "654321"
        }))
        .expect("fallback payload");
        assert_eq!(fallback.get("balance_cents"), Some(&json!(654321)));
        assert_eq!(fallback.get("initial_balance_cents"), Some(&json!(654321)));

        for (field, value) in [
            ("balanceCents", json!("12.34")),
            ("balance_cents", json!(18.49)),
            ("initialBalanceCents", json!(true)),
            ("initial_balance_cents", json!({"cents": 1})),
        ] {
            let mut payload = Map::new();
            payload.insert("name".to_string(), json!("坏账户"));
            payload.insert(field.to_string(), value);
            let error = frontend_account_to_backend(&Value::Object(payload))
            .expect_err("invalid explicit cents should be rejected");
            assert!(error.contains(field), "{error}");
            assert!(error.contains("integer cents"), "{error}");
        }
    }

    #[test]
    fn balance_discrepancy_response_uses_explicit_cents_names() {
        let value = format_account_balance_discrepancy(AccountBalanceDiscrepancy {
            account_id: 7,
            name: "招商银行".to_string(),
            old_balance_cents: 1000,
            new_balance_cents: 1250,
            diff_cents: 250,
        });

        assert_eq!(value["oldBalanceCents"], json!(1000));
        assert_eq!(value["newBalanceCents"], json!(1250));
        assert_eq!(value["diffCents"], json!(250));
        assert!(value.get("oldBalance").is_none());
    }
}
