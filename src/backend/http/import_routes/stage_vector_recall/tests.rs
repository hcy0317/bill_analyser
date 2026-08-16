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

    fn lifecycle(
        recommendation_key: &str,
        suppressed: bool,
        auto_apply_enabled: bool,
    ) -> ImportLearningLifecycleView {
        ImportLearningLifecycleView {
            recommendation_key: recommendation_key.to_string(),
            recommendation_type: "learning_rule".to_string(),
            status: if auto_apply_enabled {
                "accepted".to_string()
            } else {
                "pending".to_string()
            },
            signal_state: if auto_apply_enabled {
                "green".to_string()
            } else {
                "yellow".to_string()
            },
            accepted_count: i64::from(auto_apply_enabled),
            rejected_count: 0,
            auto_applied_count: 0,
            auto_apply_enabled,
            suppressed,
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

    #[tokio::test]
    async fn vector_recall_batch_lifecycle_reads_real_postgres_fail_closed(
    ) -> VectorRecallTestResult {
        let postgres_url = std::env::var("BILL_ANALYSER_TEST_POSTGRES_URL")
            .expect("BILL_ANALYSER_TEST_POSTGRES_URL is required for vector lifecycle batch test");
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&postgres_url)
            .await?;
        ensure_import_learning_vector_source_tables(&pool).await?;
        let unique = Utc::now().timestamp_nanos_opt().unwrap_or_default();
        let user_id: i64 =
            sqlx::query_scalar("INSERT INTO users (username, email) VALUES ($1,$2) RETURNING id")
                .bind(format!("vector-batch-{unique}"))
                .bind(format!("vector-batch-{unique}@example.test"))
                .fetch_one(&pool)
                .await?;
        let foreign_user_id: i64 =
            sqlx::query_scalar("INSERT INTO users (username, email) VALUES ($1,$2) RETURNING id")
                .bind(format!("vector-batch-foreign-{unique}"))
                .bind(format!("vector-batch-foreign-{unique}@example.test"))
                .fetch_one(&pool)
                .await?;
        let mut drafts = vec![ImportPreviewDraft {
            preview_parser_id: "alipay".to_string(),
            preview_type: "支出".to_string(),
            preview_counterparty: "咖啡店".to_string(),
            preview_description: "拿铁".to_string(),
            preview_payment_method: "支付宝".to_string(),
            ..Default::default()
        }];
        let hit = vector_hit("postgres-batch", 0.93, Some(0.07));
        let prepared = prepare_import_learning_vector_recall_hit(
            user_id,
            &drafts[0],
            &hit,
            &BTreeMap::new(),
        )
        .expect("vector candidate prepares");
        let recommendation_key = prepared.recommendation_key.clone();
        drop(prepared);
        for (fixture_user_id, status) in [(user_id, "pending"), (foreign_user_id, "accepted")] {
            sqlx::query(
                r#"
                INSERT INTO import_learning_lifecycle (
                    user_id, recommendation_key, recommendation_type, status,
                    accepted_count, auto_apply_enabled, suppressed_until
                ) VALUES ($1,$2,'expense',$3,$4,$5,NULL)
                ON CONFLICT (user_id, recommendation_key) DO UPDATE SET
                    status = excluded.status,
                    accepted_count = excluded.accepted_count,
                    auto_apply_enabled = excluded.auto_apply_enabled,
                    suppressed_until = NULL,
                    updated_at = now()
                "#,
            )
            .bind(fixture_user_id)
            .bind(&recommendation_key)
            .bind(status)
            .bind(i32::from(status == "accepted"))
            .bind(status == "accepted")
            .execute(&pool)
            .await?;
        }
        let results = vec![ImportLearningVectorRecallResult {
            draft_index: 0,
            scope_order: 0,
            hits: vec![hit],
        }];

        let recalled =
            apply_import_learning_vector_recall_results(&pool, user_id, &mut drafts, &results)
                .await?;

        assert_eq!(recalled, 1);
        assert_eq!(
            drafts[0].preview_matching_feedback["learning"]["recommendation_key"],
            recommendation_key
        );
        assert_eq!(
            drafts[0].preview_matching_feedback["learning"]["lifecycle_status"],
            "pending"
        );
        assert_eq!(
            drafts[0].preview_matching_feedback["learning"]["signal_state"],
            "yellow"
        );
        assert_eq!(
            drafts[0].preview_matching_feedback["learning"]["postgres_source_id"],
            "postgres-batch"
        );

        sqlx::query(
            "DELETE FROM import_learning_lifecycle WHERE user_id = ANY($1) AND recommendation_key = $2",
        )
        .bind(vec![user_id, foreign_user_id])
        .bind(&recommendation_key)
        .execute(&pool)
        .await?;
        sqlx::query("DELETE FROM users WHERE id = ANY($1)")
            .bind(vec![user_id, foreign_user_id])
            .execute(&pool)
            .await?;
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
    fn vector_recall_falls_back_after_missing_or_suppressed_lifecycle() {
        let draft = ImportPreviewDraft {
            preview_parser_id: "alipay".to_string(),
            preview_type: "支出".to_string(),
            preview_counterparty: "咖啡店".to_string(),
            preview_description: "拿铁".to_string(),
            preview_payment_method: "支付宝".to_string(),
            ..Default::default()
        };
        let mut high = vector_hit("high", 0.95, Some(0.05));
        high.transaction_type = Some("income".to_string());
        let low = vector_hit("low", 0.85, Some(0.15));
        let high_prepared = prepare_import_learning_vector_recall_hit(
            1,
            &draft,
            &high,
            &BTreeMap::new(),
        )
        .expect("high candidate prepares");
        let low_prepared = prepare_import_learning_vector_recall_hit(
            1,
            &draft,
            &low,
            &BTreeMap::new(),
        )
        .expect("low candidate prepares");
        let candidates = vec![high_prepared, low_prepared];

        let mut after_missing = draft.clone();
        let missing_high = BTreeMap::from([(
            candidates[1].recommendation_key.clone(),
            lifecycle(&candidates[1].recommendation_key, false, false),
        )]);
        assert!(apply_first_available_import_learning_vector_recall_candidate(
            &mut after_missing,
            &candidates,
            &missing_high,
            &[],
            &[],
        ));
        assert_eq!(
            after_missing.preview_matching_feedback["learning"]["postgres_source_id"],
            "low"
        );

        let mut after_suppressed = draft;
        let suppressed_high = BTreeMap::from([
            (
                candidates[0].recommendation_key.clone(),
                lifecycle(&candidates[0].recommendation_key, true, false),
            ),
            (
                candidates[1].recommendation_key.clone(),
                lifecycle(&candidates[1].recommendation_key, false, false),
            ),
        ]);
        assert!(apply_first_available_import_learning_vector_recall_candidate(
            &mut after_suppressed,
            &candidates,
            &suppressed_high,
            &[],
            &[],
        ));
        assert_eq!(
            after_suppressed.preview_matching_feedback["learning"]["postgres_source_id"],
            "low"
        );
    }

    #[test]
    fn vector_recall_never_auto_applies_derived_metadata() {
        let mut draft = ImportPreviewDraft {
            preview_parser_id: "alipay".to_string(),
            preview_type: "支出".to_string(),
            preview_counterparty: "咖啡店".to_string(),
            preview_description: "拿铁".to_string(),
            preview_payment_method: "支付宝".to_string(),
            ..Default::default()
        };
        let hit = vector_hit("accepted-vector", 0.91, Some(0.09));
        let prepared = prepare_import_learning_vector_recall_hit(
            1,
            &draft,
            &hit,
            &BTreeMap::new(),
        )
        .expect("candidate prepares");
        let lifecycle = lifecycle(&prepared.recommendation_key, false, true);

        assert!(apply_prepared_import_learning_vector_recall_hit(
            &mut draft,
            &prepared,
            &lifecycle,
            &[],
            &[],
        )
        .is_some());
        assert_eq!(
            draft.preview_matching_feedback["learning"]["signal_state"],
            "yellow"
        );
        assert_eq!(
            draft.preview_matching_feedback["learning"]["auto_apply"],
            false
        );
        assert_eq!(
            draft.preview_matching_feedback["learning"]["auto_apply_blocked_reason"],
            "weaviate_metadata_is_derived"
        );
        assert_eq!(draft.preview_type, "支出");
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
