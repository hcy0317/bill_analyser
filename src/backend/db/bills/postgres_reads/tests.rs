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
    fn bill_category_names_keep_payload_precedence_over_shared_path_projection() {
        assert_eq!(
            bill_category_names(
                &json!({"main_category": "自定义", "sub_category": "覆盖"}),
                Some(" 餐饮 // 工作日 / 午餐 "),
                "fallback",
            ),
            ("自定义".to_string(), "覆盖".to_string())
        );
        assert_eq!(
            bill_category_names(
                &json!({"main_category": "", "sub_category": ""}),
                Some(" 餐饮 // 工作日 / 午餐 "),
                "fallback",
            ),
            ("餐饮".to_string(), "工作日/午餐".to_string())
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

    #[test]
    fn bill_mutation_projects_balance_deltas_through_ledger_contract() {
        let mutation = PostgresBillMutation {
            occurred_at: Utc::now(),
            amount_cents: 12_000,
            direction: "expense".to_string(),
            transaction_type: "transfer".to_string(),
            source_account_id: Some(11),
            destination_account_id: Some(22),
            category_id: None,
            merchant: None,
            description: None,
            payment_method: None,
            source_hash: None,
            standard_payload: json!({"destination_amount_cents": 12_500}),
        };

        assert_eq!(
            mutation.balance_deltas().expect("valid ledger effects"),
            vec![(11, -12_000), (22, 12_500)]
        );
    }

    #[test]
    fn postgres_balance_projection_preserves_legacy_adapter_semantics() {
        assert_eq!(
            project_postgres_bill_balance_deltas(
                "investment",
                12_000,
                Some(11),
                Some(22),
                Some(0),
            )
            .expect("valid ledger effects"),
            vec![(11, -12_000), (22, 0)]
        );
        assert_eq!(
            project_postgres_bill_balance_deltas(
                "legacy-expense",
                -12_000,
                Some(11),
                Some(22),
                None,
            )
            .expect("legacy unknown types remain expense-shaped at the adapter"),
            vec![(11, -12_000)]
        );
        assert_eq!(
            project_postgres_bill_balance_deltas(
                "transfer",
                12_000,
                Some(11),
                Some(22),
                None,
            )
            .expect("missing destination amount falls back to source amount"),
            vec![(11, -12_000), (22, 12_000)]
        );
        assert!(project_postgres_bill_balance_deltas(
            "expense",
            i64::MIN,
            Some(11),
            None,
            None,
        )
        .is_err());
    }
}
