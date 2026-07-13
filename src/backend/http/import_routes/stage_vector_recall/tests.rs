#[cfg(test)]
mod vector_recall_tests {
    use super::*;
    use bill_analyser_core::WEAVIATE_DEFAULT_COLLECTION_PREFIX;
    use std::time::Duration;

    type VectorRecallTestResult = Result<(), Box<dyn std::error::Error>>;

    fn unreachable_weaviate_config() -> crate::config_weaviate::WeaviateRuntimeConfig {
        crate::config_weaviate::WeaviateRuntimeConfig {
            enabled: true,
            endpoint: Some("http://127.0.0.1:9".to_string()),
            api_key: None,
            collection_prefix: WEAVIATE_DEFAULT_COLLECTION_PREFIX.to_string(),
            timeout: Duration::from_millis(10),
            retry_attempts: 0,
            batch_size: 1,
            vector_dimensions: 4,
        }
    }

    fn disabled_weaviate_config() -> crate::config_weaviate::WeaviateRuntimeConfig {
        crate::config_weaviate::WeaviateRuntimeConfig {
            enabled: false,
            endpoint: None,
            api_key: None,
            collection_prefix: WEAVIATE_DEFAULT_COLLECTION_PREFIX.to_string(),
            timeout: Duration::from_millis(10),
            retry_attempts: 0,
            batch_size: 1,
            vector_dimensions: 4,
        }
    }

    fn vector_hit(
        postgres_source_id: &str,
        score: f64,
        distance: Option<f64>,
    ) -> WeaviateImportLearningRecallHit {
        WeaviateImportLearningRecallHit {
            postgres_source_id: postgres_source_id.to_string(),
            recommendation_key: None,
            feature_key: None,
            rule_state: Some(WEAVIATE_RULE_STATE_POSTGRES_AUTHORITATIVE.to_string()),
            transaction_type: Some("expense".to_string()),
            category_id: None,
            source_account_id: None,
            destination_account_id: None,
            payload_json: None,
            distance,
            score,
        }
    }

    #[tokio::test]
    async fn vector_recall_skips_network_without_sources() -> VectorRecallTestResult {
        let Ok(postgres_url) = std::env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
            return Ok(());
        };
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&postgres_url)
            .await?;
        ensure_import_learning_vector_source_tables(&pool).await?;
        let config = crate::config::HttpShellConfig {
            weaviate: unreachable_weaviate_config(),
            ..Default::default()
        };
        let mut drafts = vec![ImportPreviewDraft {
            preview_parser_id: "alipay".to_string(),
            preview_type: "支出".to_string(),
            preview_counterparty: "咖啡店".to_string(),
            preview_description: "拿铁".to_string(),
            preview_payment_method: "支付宝".to_string(),
            ..Default::default()
        }];

        let stats =
            apply_import_learning_vector_recall_chain(&pool, &config, i64::MAX - 17, &mut drafts)
                .await?;

        assert_eq!(stats.status, "skipped_no_vector_sources");
        assert_eq!(stats.recalled, 0);
        Ok(())
    }

    async fn ensure_import_learning_vector_source_tables(
        pool: &sqlx::PgPool,
    ) -> VectorRecallTestResult {
        bill_analyser_db::run_postgres_migrations(pool).await?;
        Ok(())
    }

    #[test]
    fn vector_recall_worker_handles_empty_request_batch() {
        let (status, results) =
            run_import_learning_vector_recall_blocking(unreachable_weaviate_config(), 1, Vec::new());

        assert_eq!(status, "searched");
        assert!(results.is_empty());
    }

    #[test]
    fn vector_recall_worker_skips_empty_hits_without_degrading() {
        let (status, results) = run_import_learning_vector_recall_blocking(
            disabled_weaviate_config(),
            1,
            vec![ImportLearningVectorRecallRequestDraft {
                draft_index: 7,
                scope_order: 0,
                features: BTreeMap::from([("counterparty".to_string(), "商户".to_string())]),
                transaction_type_scope: "支出".to_string(),
            }],
        );

        assert_eq!(status, "searched");
        assert!(results.is_empty());
    }

    #[test]
    fn vector_recall_worker_degrades_when_all_requests_fail() {
        let (status, results) = run_import_learning_vector_recall_blocking(
            unreachable_weaviate_config(),
            1,
            vec![ImportLearningVectorRecallRequestDraft {
                draft_index: 7,
                scope_order: 0,
                features: BTreeMap::from([
                    ("counterparty".to_string(), "商户".to_string()),
                    ("description".to_string(), "早餐".to_string()),
                    ("payment_method".to_string(), "支付宝".to_string()),
                ]),
                transaction_type_scope: "支出".to_string(),
            }],
        );

        assert!(status.starts_with("degraded:"), "status={status}");
        assert!(results.is_empty());
    }

    #[test]
    fn vector_recall_candidates_prefer_highest_score_across_scopes() {
        let low_income = vector_hit("income-low", 0.72, Some(0.28));
        let high_investment = vector_hit("investment-high", 0.91, Some(0.09));
        let medium_expense = vector_hit("expense-medium", 0.86, Some(0.14));
        let mut candidates = vec![
            (0, &low_income),
            (2, &high_investment),
            (1, &medium_expense),
        ];

        sort_import_learning_vector_recall_candidates(&mut candidates);

        assert_eq!(candidates[0].1.postgres_source_id, "investment-high");
        assert_eq!(candidates[1].1.postgres_source_id, "expense-medium");
        assert_eq!(candidates[2].1.postgres_source_id, "income-low");
    }

    #[test]
    fn vector_recall_requests_fan_out_non_transfer_scopes() {
        let config = crate::config::HttpShellConfig {
            weaviate: disabled_weaviate_config(),
            ..Default::default()
        };
        let drafts = vec![ImportPreviewDraft {
            preview_parser_id: "alipay".to_string(),
            preview_type: "支出".to_string(),
            preview_counterparty: "基金平台".to_string(),
            preview_description: "定投扣款".to_string(),
            preview_payment_method: "招商卡".to_string(),
            ..Default::default()
        }];

        let requests = build_import_learning_vector_recall_requests(&config, 1, &drafts);

        assert_eq!(
            requests
                .iter()
                .map(|request| request.transaction_type_scope.as_str())
                .collect::<Vec<_>>(),
            vec!["income", "expense", "investment"]
        );
        assert_eq!(
            requests
                .iter()
                .map(|request| request.scope_order)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn vector_recall_requests_keep_transfer_scope_only_for_authorized_transfer() {
        let config = crate::config::HttpShellConfig {
            weaviate: disabled_weaviate_config(),
            ..Default::default()
        };
        let drafts = vec![ImportPreviewDraft {
            preview_parser_id: "alipay".to_string(),
            preview_type: "转账".to_string(),
            preview_counterparty: "支付宝".to_string(),
            preview_description: "余额转出".to_string(),
            preview_payment_method: "支付宝".to_string(),
            preview_matching_feedback: json!({
                "transfer": {
                    "candidate_type": "transfer",
                    "review_status": "pending"
                }
            }),
            ..Default::default()
        }];

        let requests = build_import_learning_vector_recall_requests(&config, 1, &drafts);

        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].transaction_type_scope, "transfer");
    }
}
