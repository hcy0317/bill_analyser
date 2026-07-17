mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn explicit_category_id_from_fields_accepts_canonical_identity_keys() {
        let mut fields = BillRecord::new();
        assert_eq!(explicit_category_id_from_fields(&fields).unwrap(), None);

        fields.insert("category_id".to_string(), json!(42));
        assert_eq!(explicit_category_id_from_fields(&fields).unwrap(), Some(42));

        fields.clear();
        fields.insert("categoryId".to_string(), json!("43"));
        assert_eq!(explicit_category_id_from_fields(&fields).unwrap(), Some(43));

        fields.insert("category_id".to_string(), json!(0));
        assert!(explicit_category_id_from_fields(&fields).is_err());
    }

    #[test]
    fn identity_field_and_type_helpers_cover_invalid_edges() {
        let mut fields = BillRecord::new();
        fields.insert("source_account_id".to_string(), Value::Null);
        assert_eq!(
            optional_positive_id_from_fields(&fields, &["source_account_id"]).unwrap(),
            None
        );

        fields.insert("source_account_id".to_string(), json!("abc"));
        assert!(optional_positive_id_from_fields(&fields, &["source_account_id"]).is_err());

        fields.insert("source_account_id".to_string(), json!(-1));
        assert!(optional_positive_id_from_fields(&fields, &["source_account_id"]).is_err());

        fields.clear();
        assert_eq!(
            optional_positive_id_from_fields(&fields, &["source_account_id"]).unwrap(),
            None
        );

        assert!(postgres_category_type_matches_transaction_type(
            None, "支出"
        ));
        assert!(postgres_category_type_matches_transaction_type(
            Some(0),
            "支出"
        ));
        assert!(postgres_category_type_matches_transaction_type(
            Some(4),
            "transfer"
        ));
        assert!(postgres_category_type_matches_transaction_type(
            Some(5),
            "投资"
        ));
        assert!(!postgres_category_type_matches_transaction_type(
            Some(2),
            "支出"
        ));
        assert_eq!(postgres_category_type_code("转账"), Some(4));
        assert_eq!(postgres_category_type_code("investment"), Some(5));
        assert_eq!(postgres_category_type_code("unknown"), None);
        assert_eq!(postgres_transaction_type_code("4"), Some(4));
        assert_eq!(postgres_transaction_type_code("投资"), Some(5));
        assert_eq!(postgres_transaction_type_code("unknown"), None);
    }

    #[test]
    fn amount_cents_fields_drive_hash_and_initial_balance_helpers() {
        let fields = BillRecord::from_iter([("amount_cents".to_string(), json!(12345))]);
        assert!(update_requires_hash_recalculation(&fields));

        let legacy = BillRecord::from_iter([("amount".to_string(), json!(123.45))]);
        assert!(!update_requires_hash_recalculation(&legacy));

        assert_eq!(
            destination_amount_cents(&json!({"destination_amount_cents": "54321"})),
            Some(54321)
        );
        assert_eq!(
            destination_amount_cents(&json!({"destination_amount": "543.21"})),
            None
        );
        assert_eq!(
            destination_amount_cents(&json!({"destinationAmount": 543.21})),
            None
        );
        assert_eq!(
            postgres_initial_balance_money(&json!({"initial_balance_cents": 98765}), 100)
                .expect("number initial balance")
                .to_cents(),
            98765
        );
        assert_eq!(
            postgres_initial_balance_money(&json!({"initial_balance_cents": "12345"}), 100)
                .expect("text initial balance")
                .to_cents(),
            12345
        );
        assert!(
            postgres_initial_balance_money(&json!({"initial_balance_cents": "12.34"}), 100)
                .is_err()
        );
        assert_eq!(
            postgres_initial_balance_money(&json!({"initial_balance": "12.34"}), 100)
                .expect("legacy yuan initial balance is ignored at runtime")
                .to_cents(),
            100
        );
    }

    #[test]
    fn bill_mutation_rejects_i64_min_amount_without_panicking() {
        let fields = BillRecord::from_iter([
            ("date".to_string(), json!("2026-07-16 10:00:00")),
            ("type".to_string(), json!("支出")),
            ("amount_cents".to_string(), json!(i64::MIN)),
        ]);

        let error = prepare_postgres_bill_mutation(&fields)
            .expect_err("i64::MIN cannot become a positive PostgreSQL bill amount");
        assert!(matches!(
            error,
            DbError::InvalidOperation(message) if message == "invalid bill amount_cents"
        ));
    }
}
