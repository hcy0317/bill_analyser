#[cfg(test)]
mod stage_learning_transfer_tests {
    use super::*;

    fn seed_learning_lifecycle_status(
        connection: &Connection,
        recommendation_key: &str,
        status: &str,
    ) {
        connection
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS import_learning_lifecycle (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_id INTEGER NOT NULL DEFAULT 1,
                    recommendation_key TEXT NOT NULL,
                    recommendation_type TEXT NOT NULL DEFAULT 'import_preview',
                    status TEXT NOT NULL DEFAULT 'yellow',
                    accepted_count INTEGER NOT NULL DEFAULT 0,
                    rejected_count INTEGER NOT NULL DEFAULT 0,
                    auto_applied_count INTEGER NOT NULL DEFAULT 0,
                    auto_apply_enabled INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL DEFAULT '',
                    updated_at TEXT NOT NULL DEFAULT '',
                    UNIQUE(user_id, recommendation_key)
                );
                ",
            )
            .expect("learning lifecycle schema");
        connection
            .execute(
                "
                INSERT INTO import_learning_lifecycle (
                    user_id, recommendation_key, recommendation_type, status,
                    accepted_count, rejected_count, auto_applied_count, auto_apply_enabled,
                    created_at, updated_at
                ) VALUES (42, ?1, 'import_preview', ?2, 3, 0, 0, 1, '', '')
                ON CONFLICT(user_id, recommendation_key) DO UPDATE SET
                    status = excluded.status,
                    accepted_count = excluded.accepted_count,
                    auto_apply_enabled = excluded.auto_apply_enabled
                ",
                params![recommendation_key, status],
            )
            .expect("learning lifecycle row");
    }

    #[test]
    fn transfer_learning_domain_allows_account_only_rule() {
        let features = build_composite_match_features("cmbc", "微信零钱", "账户互转", "网络银行")
            .expect("transfer features");
        let rule = ImportIntelligenceLearningRule {
            id: 1,
            parser_id: "cmbc".to_string(),
            composite_hash: composite_hash_from_features(&features),
            match_features: features,
            learned_type: None,
            learned_category_id: None,
            learned_source_account_id: Some(100),
            learned_destination_account_id: Some(200),
        };
        let connection = Connection::open_in_memory().expect("connection");
        let mut draft = ImportPreviewDraft {
            preview_type: "转账".to_string(),
            preview_parser_id: "cmbc".to_string(),
            preview_counterparty: "微信零钱".to_string(),
            preview_description: "账户互转".to_string(),
            preview_payment_method: "网络银行".to_string(),
            dedup_type: Some("transfer".to_string()),
            preview_matching_feedback: json!({
                "transfer": {"candidate_type": "transfer"}
            }),
            ..ImportPreviewDraft::default()
        };

        let result = apply_learning_rule_match(
            &connection,
            42,
            &mut draft,
            std::slice::from_ref(&rule),
            &BTreeMap::new(),
            &[],
            &[
                json!({"id": 100, "name": "民生银行"}),
                json!({"id": 200, "name": "微信零钱"}),
            ],
        )
        .expect("transfer account-only learning");

        assert_eq!(result.rule_id, Some(1));
        assert_eq!(draft.preview_type, "转账");
        assert_eq!(
            draft
                .preview_matching_feedback
                .pointer("/learning/summary")
                .and_then(Value::as_str),
            Some("民生银行")
        );
    }

    #[test]
    fn transfer_learning_does_not_override_source_chain_accounts() {
        let features = build_composite_match_features(
            "cmbc",
            "支付宝（中国）网络技术有限公司客户备付金",
            "支付宝快捷支付",
            "网络银行",
        )
        .expect("transfer learning features");
        let composite_hash = composite_hash_from_features(&features);
        let rule = ImportIntelligenceLearningRule {
            id: 7103,
            parser_id: "cmbc".to_string(),
            composite_hash,
            match_features: features,
            learned_type: None,
            learned_category_id: None,
            learned_source_account_id: Some(9001),
            learned_destination_account_id: Some(9002),
        };
        let mut draft = ImportPreviewDraft {
            preview_type: "转账".to_string(),
            preview_parser_id: "cmbc".to_string(),
            preview_counterparty: "支付宝（中国）网络技术有限公司客户备付金".to_string(),
            preview_description: "支付宝快捷支付".to_string(),
            preview_payment_method: "网络银行".to_string(),
            preview_source_account_id: Some(1001),
            preview_destination_account_id: Some(1002),
            dedup_type: Some("transfer".to_string()),
            preview_matching_feedback: json!({
                "transfer": {"candidate_type": "transfer"}
            }),
            ..ImportPreviewDraft::default()
        };
        let account_values = vec![
            json!({"id": 1001, "name": "民生银行"}),
            json!({"id": 1002, "name": "支付宝"}),
            json!({"id": 9001, "name": "旧来源"}),
            json!({"id": 9002, "name": "旧目标"}),
        ];

        let connection = Connection::open_in_memory().expect("connection");
        let pending_learning = apply_learning_rule_match(
            &connection,
            42,
            &mut draft,
            std::slice::from_ref(&rule),
            &BTreeMap::new(),
            &[],
            &account_values,
        );
        assert!(pending_learning.is_none());
        assert_eq!(draft.preview_source_account_id, Some(1001));
        assert_eq!(draft.preview_destination_account_id, Some(1002));
    }

    #[test]
    fn transfer_learning_ignores_conflicting_type_but_keeps_transfer_category_and_accounts() {
        let features = build_composite_match_features(
            "cmbc",
            "支付宝（中国）网络技术有限公司客户备付金",
            "支付宝快捷支付",
            "网络银行",
        )
        .expect("transfer learning features");
        let composite_hash = composite_hash_from_features(&features);
        let transfer_category = ImportIntelligenceCategory {
            id: 4400,
            type_code: 4,
            main_category: "内部转账".to_string(),
            sub_category: "账户互转".to_string(),
        };
        let categories_by_id = [(transfer_category.id, transfer_category.clone())]
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        let category_values = vec![json!({
            "id": transfer_category.id,
            "main_category": transfer_category.main_category,
            "sub_category": transfer_category.sub_category,
            "type": transfer_category.type_code,
        })];
        let account_values = vec![
            json!({"id": 9001, "name": "民生银行"}),
            json!({"id": 9002, "name": "支付宝"}),
        ];
        let rule = ImportIntelligenceLearningRule {
            id: 7104,
            parser_id: "cmbc".to_string(),
            composite_hash,
            match_features: features,
            learned_type: Some("支出".to_string()),
            learned_category_id: Some(4400),
            learned_source_account_id: Some(9001),
            learned_destination_account_id: Some(9002),
        };
        let mut draft = ImportPreviewDraft {
            preview_type: "转账".to_string(),
            preview_parser_id: "cmbc".to_string(),
            preview_counterparty: "支付宝（中国）网络技术有限公司客户备付金".to_string(),
            preview_description: "支付宝快捷支付".to_string(),
            preview_payment_method: "网络银行".to_string(),
            dedup_type: Some("transfer".to_string()),
            preview_matching_feedback: json!({
                "transfer": {"candidate_type": "transfer"}
            }),
            ..ImportPreviewDraft::default()
        };

        let connection = Connection::open_in_memory().expect("connection");
        let pending_learning = apply_learning_rule_match(
            &connection,
            42,
            &mut draft,
            std::slice::from_ref(&rule),
            &categories_by_id,
            &category_values,
            &account_values,
        )
        .expect("pending learning match");
        assert_eq!(pending_learning.rule_id, Some(7104));
        assert_eq!(
            draft
                .preview_matching_feedback
                .pointer("/learning/recommended_type")
                .and_then(Value::as_str),
            Some("转账")
        );
        assert_eq!(draft.preview_type, "转账");
        assert_eq!(draft.preview_source_account_id, None);

        seed_learning_lifecycle_status(&connection, &pending_learning.recommendation_key, "green");
        let green_learning = apply_learning_rule_match(
            &connection,
            42,
            &mut draft,
            std::slice::from_ref(&rule),
            &categories_by_id,
            &category_values,
            &account_values,
        )
        .expect("green learning match");
        assert!(green_learning.auto_applied);
        assert_eq!(draft.preview_type, "转账");
        assert_eq!(draft.preview_main_category, "内部转账");
        assert_eq!(draft.preview_sub_category, "账户互转");
        assert_eq!(draft.preview_source_account_id, Some(9001));
        assert_eq!(draft.preview_destination_account_id, Some(9002));
    }
}
