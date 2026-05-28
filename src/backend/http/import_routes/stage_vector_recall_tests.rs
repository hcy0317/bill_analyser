#[cfg(test)]
mod stage_vector_recall_tests {
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

    fn seed_vector_recall_taxonomy(connection: &Connection) {
        connection
            .execute_batch(
                r#"
                CREATE TABLE categories (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    type INTEGER NOT NULL,
                    main_category TEXT NOT NULL,
                    sub_category TEXT NOT NULL,
                    priority INTEGER DEFAULT 0
                );
                CREATE TABLE accounts (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    name TEXT NOT NULL,
                    aliases TEXT,
                    hidden INTEGER DEFAULT 0
                );
                INSERT INTO categories(id, user_id, type, main_category, sub_category, priority)
                VALUES
                    (11, 42, 3, '餐饮', '咖啡', 1),
                    (44, 42, 4, '内部转账', '账户互转', 2);
                INSERT INTO accounts(id, user_id, name, aliases, hidden)
                VALUES
                    (100, 42, '支付宝', '["alipay"]', 0),
                    (200, 42, '微信零钱', '["wallet"]', 0);
                "#,
            )
            .expect("vector recall taxonomy schema");
    }

    fn vector_recall_hit(
        score: f64,
        transaction_type: &str,
        category_id: Option<i64>,
        source_account_id: Option<i64>,
        destination_account_id: Option<i64>,
    ) -> WeaviateImportLearningRecallHit {
        WeaviateImportLearningRecallHit {
            postgres_source_id: "feature:vector".to_string(),
            recommendation_key: Some("rk-vector".to_string()),
            feature_key: Some("counterparty".to_string()),
            rule_state: Some(WEAVIATE_RULE_STATE_POSTGRES_AUTHORITATIVE.to_string()),
            transaction_type: Some(transaction_type.to_string()),
            category_id,
            source_account_id,
            destination_account_id,
            payload_json: None,
            distance: Some(1.0 - score),
            score,
        }
    }

    #[test]
    fn vector_recall_metadata_cannot_enable_auto_apply_without_authoritative_projection() {
        let category = ImportIntelligenceCategory {
            id: 11,
            type_code: 3,
            main_category: "餐饮".to_string(),
            sub_category: "咖啡".to_string(),
        };
        let categories_by_id = [(category.id, category.clone())]
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        let category_values = vec![json!({
            "id": category.id,
            "main_category": category.main_category,
            "sub_category": category.sub_category,
            "type": category.type_code,
        })];
        let account_values = vec![json!({"id": 100, "name": "支付宝"})];
        let hit = WeaviateImportLearningRecallHit {
            postgres_source_id: "feature:42".to_string(),
            recommendation_key: Some("rk-vector".to_string()),
            feature_key: Some("counterparty".to_string()),
            rule_state: Some(WEAVIATE_RULE_STATE_POSTGRES_AUTHORITATIVE.to_string()),
            transaction_type: Some("expense".to_string()),
            category_id: Some(11),
            source_account_id: Some(100),
            destination_account_id: None,
            payload_json: None,
            distance: Some(0.10),
            score: 0.90,
        };
        let mut draft = ImportPreviewDraft {
            preview_type: "支出".to_string(),
            preview_parser_id: "alipay".to_string(),
            preview_counterparty: "咖啡店".to_string(),
            preview_description: "拿铁".to_string(),
            preview_payment_method: "支付宝".to_string(),
            preview_amount: 32.0,
            ..ImportPreviewDraft::default()
        };
        let connection = Connection::open_in_memory().expect("connection");

        apply_import_learning_vector_recall_hit(
            &connection,
            42,
            &mut draft,
            &hit,
            &categories_by_id,
            &category_values,
            &account_values,
        )
        .expect("yellow vector signal");
        let recommendation_key = draft
            .preview_matching_feedback
            .pointer("/learning/recommendation_key")
            .and_then(Value::as_str)
            .expect("recommendation key")
            .to_string();
        assert_eq!(draft.preview_main_category, "");
        seed_learning_lifecycle_status(&connection, &recommendation_key, "green");
        draft
            .preview_matching_feedback
            .as_object_mut()
            .expect("feedback object")
            .remove("learning");

        apply_import_learning_vector_recall_hit(
            &connection,
            42,
            &mut draft,
            &hit,
            &categories_by_id,
            &category_values,
            &account_values,
        )
        .expect("green lifecycle vector signal");

        let learning = draft
            .preview_matching_feedback
            .get("learning")
            .and_then(Value::as_object)
            .expect("learning signal");
        assert_eq!(learning["source"], "weaviate_vector_recall");
        assert_eq!(learning["auto_apply"], false);
        assert_eq!(
            learning["auto_apply_blocked_reason"],
            "weaviate_metadata_is_derived"
        );
        assert_eq!(learning["signal_state"], "yellow");
        assert_eq!(draft.preview_main_category, "");
        assert_eq!(draft.preview_source_account_id, None);
    }

    #[test]
    fn vector_recall_chain_skips_empty_feature_requests_when_enabled() -> rusqlite::Result<()> {
        let connection = Connection::open_in_memory()?;
        let mut config = HttpShellConfig::default();
        config.weaviate.enabled = true;
        config.weaviate.endpoint = Some("http://127.0.0.1:1".to_string());
        let mut drafts = vec![ImportPreviewDraft {
            preview_type: "支出".to_string(),
            ..ImportPreviewDraft::default()
        }];

        let stats =
            apply_import_learning_vector_recall_chain(&connection, &config, 42, &mut drafts)?;

        assert_eq!(stats.status, "skipped_empty");
        assert_eq!(stats.recalled, 0);
        Ok(())
    }

    #[test]
    fn vector_recall_results_apply_first_valid_hit_and_skip_existing_feedback(
    ) -> rusqlite::Result<()> {
        let connection = Connection::open_in_memory()?;
        seed_vector_recall_taxonomy(&connection);
        let mut drafts = vec![
            ImportPreviewDraft {
                preview_type: "支出".to_string(),
                preview_parser_id: "alipay".to_string(),
                preview_counterparty: "咖啡店".to_string(),
                preview_description: "拿铁".to_string(),
                preview_payment_method: "支付宝".to_string(),
                preview_amount: 32.0,
                ..ImportPreviewDraft::default()
            },
            ImportPreviewDraft {
                preview_matching_feedback: json!({"learning": {"source": "rule"}}),
                ..ImportPreviewDraft::default()
            },
        ];
        let valid_hit = vector_recall_hit(0.91, "expense", Some(11), Some(100), None);
        let low_score_hit = vector_recall_hit(0.60, "expense", Some(11), Some(100), None);
        let results = vec![
            ImportLearningVectorRecallResult {
                draft_index: 99,
                hits: vec![valid_hit.clone()],
            },
            ImportLearningVectorRecallResult {
                draft_index: 1,
                hits: vec![valid_hit.clone()],
            },
            ImportLearningVectorRecallResult {
                draft_index: 0,
                hits: vec![low_score_hit, valid_hit],
            },
        ];

        let applied =
            apply_import_learning_vector_recall_results(&connection, 42, &mut drafts, &results)?;

        assert_eq!(applied, 1);
        let learning = drafts[0]
            .preview_matching_feedback
            .get("learning")
            .and_then(Value::as_object)
            .expect("vector learning feedback");
        assert_eq!(learning["source"], "weaviate_vector_recall");
        assert_eq!(
            learning
                .get("applied_preview")
                .and_then(|value| value.pointer("/preview_main_category"))
                .and_then(Value::as_str),
            Some("餐饮")
        );
        assert_eq!(
            learning
                .get("applied_preview")
                .and_then(|value| value.pointer("/preview_source_account_id"))
                .and_then(Value::as_i64),
            Some(100)
        );
        assert_eq!(learning["auto_apply"], false);
        assert!(drafts[1]
            .preview_matching_feedback
            .pointer("/learning/source")
            .is_some());
        Ok(())
    }

    #[test]
    fn vector_recall_transfer_rejects_conflicting_non_transfer_metadata() {
        let expense_category = ImportIntelligenceCategory {
            id: 11,
            type_code: 3,
            main_category: "餐饮".to_string(),
            sub_category: "咖啡".to_string(),
        };
        let categories_by_id = [(expense_category.id, expense_category)]
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        let hit = vector_recall_hit(0.91, "expense", Some(11), None, None);
        let mut draft = ImportPreviewDraft {
            preview_type: "转账".to_string(),
            preview_parser_id: "cmbc".to_string(),
            preview_counterparty: "支付宝".to_string(),
            preview_description: "账户互转".to_string(),
            preview_payment_method: "网络银行".to_string(),
            dedup_type: Some("transfer".to_string()),
            preview_matching_feedback: json!({
                "transfer": {"candidate_type": "transfer"}
            }),
            ..ImportPreviewDraft::default()
        };
        let connection = Connection::open_in_memory().expect("connection");

        assert!(apply_import_learning_vector_recall_hit(
            &connection,
            42,
            &mut draft,
            &hit,
            &categories_by_id,
            &[],
            &[],
        )
        .is_none());
        assert!(draft.preview_matching_feedback.get("learning").is_none());
    }

    #[test]
    fn vector_recall_account_only_noop_is_ignored() {
        let mut hit = vector_recall_hit(0.91, "expense", None, Some(100), None);
        hit.transaction_type = None;
        let mut draft = ImportPreviewDraft {
            preview_type: "支出".to_string(),
            preview_source_account_id: Some(100),
            preview_parser_id: "alipay".to_string(),
            preview_counterparty: "咖啡店".to_string(),
            preview_description: "拿铁".to_string(),
            preview_payment_method: "支付宝".to_string(),
            ..ImportPreviewDraft::default()
        };
        let connection = Connection::open_in_memory().expect("connection");

        assert!(apply_import_learning_vector_recall_hit(
            &connection,
            42,
            &mut draft,
            &hit,
            &BTreeMap::new(),
            &[],
            &[json!({"id": 100, "name": "支付宝"})],
        )
        .is_none());
        assert!(draft.preview_matching_feedback.get("learning").is_none());
    }
}
