#[derive(Debug, Clone, Default)]
struct ImportLearningVectorRecallStats {
    status: String,
    recalled: usize,
}

#[derive(Debug, Clone)]
struct ImportLearningVectorRecallResult {
    draft_index: usize,
    hits: Vec<WeaviateImportLearningRecallHit>,
}

const IMPORT_VECTOR_RECALL_MAX_CONCURRENCY: usize = 8;

async fn apply_import_learning_vector_recall_chain(
    connection: &Connection,
    config: &HttpShellConfig,
    user_id: i64,
    drafts: &mut [ImportPreviewDraft],
) -> Result<ImportLearningVectorRecallStats, bill_analyser_db::DbError> {
    if !config.weaviate.enabled {
        return Ok(ImportLearningVectorRecallStats {
            status: "disabled".to_string(),
            recalled: 0,
        });
    }
    if !has_import_learning_feature_vector_sources(connection, user_id).await? {
        return Ok(ImportLearningVectorRecallStats {
            status: "skipped_no_vector_sources".to_string(),
            recalled: 0,
        });
    }
    let requests = build_import_learning_vector_recall_requests(config, user_id, drafts);
    if requests.is_empty() {
        return Ok(ImportLearningVectorRecallStats {
            status: "skipped_empty".to_string(),
            recalled: 0,
        });
    }

    let (status, results) =
        run_import_learning_vector_recall_blocking(config.weaviate.clone(), user_id, requests);
    let recalled =
        apply_import_learning_vector_recall_results(connection, user_id, drafts, &results).await?;
    Ok(ImportLearningVectorRecallStats { status, recalled })
}

fn build_import_learning_vector_recall_requests(
    config: &HttpShellConfig,
    user_id: i64,
    drafts: &[ImportPreviewDraft],
) -> Vec<ImportLearningVectorRecallRequestDraft> {
    drafts
        .iter()
        .enumerate()
        .filter(|(_, draft)| draft.preview_matching_feedback.get("learning").is_none())
        .filter_map(|(draft_index, draft)| {
            let features = build_composite_match_features(
                &draft.preview_parser_id,
                &draft.preview_counterparty,
                &draft.preview_description,
                &draft.preview_payment_method,
            )?;
            let transaction_type_scope =
                normalize_weaviate_transaction_type_scope(&draft.preview_type);
            if build_import_learning_vector_recall_queries(
                &config.weaviate.collection_prefix,
                user_id,
                &features,
                &transaction_type_scope,
                WEAVIATE_RECALL_DEFAULT_LIMIT,
            )
            .is_empty()
            {
                return None;
            }
            Some(ImportLearningVectorRecallRequestDraft {
                draft_index,
                features,
                transaction_type_scope,
            })
        })
        .collect()
}

fn run_import_learning_vector_recall_blocking(
    config: crate::config_weaviate::WeaviateRuntimeConfig,
    user_id: i64,
    requests: Vec<ImportLearningVectorRecallRequestDraft>,
) -> (String, Vec<ImportLearningVectorRecallResult>) {
    let worker = std::thread::Builder::new()
        .name("bill-import-weaviate-recall".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .enable_time()
                .build()
                .map_err(|error| error.to_string())?;
            runtime.block_on(async move {
                let mut results = Vec::new();
                let mut errors = 0usize;
                let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(
                    IMPORT_VECTOR_RECALL_MAX_CONCURRENCY,
                ));
                let mut tasks = tokio::task::JoinSet::new();
                for request in requests {
                    let config = config.clone();
                    let semaphore = semaphore.clone();
                    tasks.spawn(async move {
                        let _permit = semaphore
                            .acquire_owned()
                            .await
                            .map_err(|error| error.to_string())?;
                        let recall_request = WeaviateImportLearningRecallRequest {
                            user_id,
                            features: request.features,
                            transaction_type_scope: request.transaction_type_scope,
                            limit: WEAVIATE_RECALL_DEFAULT_LIMIT,
                        };
                        recall_import_learning_candidates(&config, &recall_request)
                            .await
                            .map(|hits| ImportLearningVectorRecallResult {
                                draft_index: request.draft_index,
                                hits,
                            })
                            .map_err(|error| error.to_string())
                    });
                }
                while let Some(joined) = tasks.join_next().await {
                    match joined {
                        Ok(Ok(result)) if result.hits.is_empty() => {}
                        Ok(Ok(result)) => results.push(result),
                        Ok(Err(_)) | Err(_) => errors += 1,
                    }
                }
                results.sort_by_key(|result| result.draft_index);
                if errors > 0 && results.is_empty() {
                    return Err(format!("{} recall requests failed", errors));
                }
                if errors > 0 {
                    tracing::warn!(
                        domain = "import_parser",
                        operation = "import_learning_vector_recall",
                        failed_requests = errors,
                        "some import learning vector recall requests failed"
                    );
                }
                Ok::<_, String>(results)
            })
        });

    let Ok(worker) = worker else {
        return ("degraded:spawn_failed".to_string(), Vec::new());
    };
    match worker.join() {
        Ok(Ok(results)) => ("searched".to_string(), results),
        Ok(Err(error)) => (format!("degraded:{error}"), Vec::new()),
        Err(_) => ("degraded:worker_failed".to_string(), Vec::new()),
    }
}

async fn apply_import_learning_vector_recall_results(
    connection: &Connection,
    user_id: i64,
    drafts: &mut [ImportPreviewDraft],
    results: &[ImportLearningVectorRecallResult],
) -> Result<usize, bill_analyser_db::DbError> {
    if results.is_empty() {
        return Ok(0);
    }
    let categories = load_import_intelligence_categories(connection, user_id).await?;
    let categories_by_id = categories
        .iter()
        .cloned()
        .map(|category| (category.id, category))
        .collect::<BTreeMap<_, _>>();
    let category_values = categories
        .iter()
        .map(|category| {
            json!({
                "id": category.id,
                "main_category": category.main_category,
                "sub_category": category.sub_category,
                "type": category.type_code,
            })
        })
        .collect::<Vec<_>>();
    let account_values = load_import_intelligence_accounts(connection, user_id)
        .await?
        .iter()
        .map(|account| json!({"id": account.id, "name": account.name}))
        .collect::<Vec<_>>();

    let mut applied = 0;
    for result in results {
        let Some(draft) = drafts.get_mut(result.draft_index) else {
            continue;
        };
        if draft.preview_matching_feedback.get("learning").is_some() {
            continue;
        }
        for hit in &result.hits {
            if apply_import_learning_vector_recall_hit(
                connection,
                user_id,
                draft,
                hit,
                &categories_by_id,
                &category_values,
                &account_values,
            )
            .is_some()
            {
                applied += 1;
                break;
            }
        }
    }
    Ok(applied)
}

fn apply_import_learning_vector_recall_hit(
    connection: &Connection,
    user_id: i64,
    draft: &mut ImportPreviewDraft,
    hit: &WeaviateImportLearningRecallHit,
    categories_by_id: &BTreeMap<i64, ImportIntelligenceCategory>,
    category_values: &[Value],
    account_values: &[Value],
) -> Option<()> {
    if hit
        .rule_state
        .as_deref()
        .filter(|state| *state == WEAVIATE_RULE_STATE_POSTGRES_AUTHORITATIVE)
        .is_none()
        || hit.score < 0.70
    {
        return None;
    }
    let transfer_protected = is_transfer_protected_preview(draft);
    let raw_learned_type = hit
        .transaction_type
        .as_deref()
        .and_then(normalize_transaction_type_text);
    let learned_type = if transfer_protected {
        raw_learned_type
            .as_deref()
            .filter(|transaction_type| *transaction_type == "转账")
            .map(ToOwned::to_owned)
    } else {
        raw_learned_type
    };
    let candidate_preview_type = if transfer_protected {
        "转账"
    } else {
        learned_type
            .as_deref()
            .unwrap_or_else(|| draft.preview_type.trim())
    };
    let learned_category = hit.category_id.and_then(|category_id| {
        let category = categories_by_id.get(&category_id)?;
        if transfer_protected && category.type_code != 4 {
            return None;
        }
        category_type_matches_preview(category.type_code, candidate_preview_type).then_some(category)
    });
    let recommended_category_id = learned_category.map(|category| category.id);
    if learned_type.is_none()
        && recommended_category_id.is_none()
        && hit.source_account_id.is_none()
        && hit.destination_account_id.is_none()
    {
        return None;
    }

    let vector_rule = ImportIntelligenceLearningRule {
        id: 0,
        parser_id: String::new(),
        composite_hash: String::new(),
        match_features: BTreeMap::new(),
        learned_type: learned_type.clone(),
        learned_category_id: recommended_category_id,
        learned_source_account_id: hit.source_account_id,
        learned_destination_account_id: hit.destination_account_id,
    };
    let mut recommended_draft = draft.clone();
    apply_learning_rule_projection(
        &mut recommended_draft,
        learned_type.as_deref(),
        learned_category,
        &vector_rule,
        transfer_protected,
    );
    if learned_type.is_none()
        && recommended_category_id.is_none()
        && import_preview_stage2_snapshot(&recommended_draft) == import_preview_stage2_snapshot(draft)
    {
        return None;
    }
    let recommendation_key = build_import_learning_recommendation_key(
        &ImportLearningRecommendationKeyInput {
            user_id,
            recommended_type: learned_type
                .clone()
                .unwrap_or_else(|| recommended_draft.preview_type.clone()),
            recommended_category_id,
            recommended_source_account_id: hit.source_account_id,
            recommended_destination_account_id: hit.destination_account_id,
            transaction_type_scope: draft.preview_type.clone(),
            parser_bucket: draft.preview_parser_id.clone(),
            counterparty_bucket: draft.preview_counterparty.clone(),
            payment_bucket: draft.preview_payment_method.clone(),
            description_bucket: draft.preview_description.clone(),
            amount_bucket: Some(
                amount_cents_bucket(Some(&json!(draft.preview_amount_cents))).to_string(),
            ),
            transfer_protected,
            ..ImportLearningRecommendationKeyInput::default()
        },
    );
    let lifecycle_user_id = u64::try_from(user_id).ok().and_then(|value| UserId::new(value).ok())?;
    let lifecycle = get_import_learning_lifecycle_view(
        connection,
        lifecycle_user_id,
        &recommendation_key,
    )
    .ok()
    .flatten()?;
    if lifecycle.suppressed {
        return None;
    }
    let mut rule_payload = Map::new();
    rule_payload.insert("learned_type".to_string(), json!(learned_type));
    rule_payload.insert(
        "learned_category_id".to_string(),
        json!(recommended_category_id),
    );
    rule_payload.insert(
        "learned_source_account_id".to_string(),
        json!(hit.source_account_id),
    );
    rule_payload.insert(
        "learned_destination_account_id".to_string(),
        json!(hit.destination_account_id),
    );
    let previous_preview = import_preview_stage2_snapshot(draft);
    let applied_preview = import_preview_stage2_snapshot(&recommended_draft);
    let recommended_type_value = applied_preview
        .get("preview_type")
        .cloned()
        .unwrap_or_else(|| json!(draft.preview_type.clone()));
    matching_feedback_object_mut(draft).insert(
        "learning".to_string(),
        json!({
            "rule_id": Value::Null,
            "score": hit.score,
            "mode": "vector",
            "reason": format!(
                "weaviate vector recall | {} | distance={}",
                hit.postgres_source_id,
                hit.distance
                    .map(|distance| format!("{distance:.4}"))
                    .unwrap_or_else(|| "unknown".to_string())
            ),
            "summary": build_learning_rule_result_summary(&rule_payload, category_values, account_values),
            "recommended_type": recommended_type_value,
            "review_status": "pending",
            "auto_apply": false,
            "auto_apply_blocked_reason": if lifecycle.auto_apply_enabled {
                "weaviate_metadata_is_derived"
            } else {
                ""
            },
            "source": "weaviate_vector_recall",
            "recommendation_key": recommendation_key,
            "vector_recommendation_key": hit.recommendation_key,
            "postgres_source_id": hit.postgres_source_id,
            "feature_key": hit.feature_key,
            "lifecycle_status": lifecycle.status.clone(),
            "signal_state": if lifecycle.auto_apply_enabled {
                "yellow".to_string()
            } else {
                lifecycle.signal_state.clone()
            },
            "accepted_count": lifecycle.accepted_count,
            "rejected_count": lifecycle.rejected_count,
            "auto_applied_count": lifecycle.auto_applied_count,
            "previous_preview": previous_preview,
            "applied_preview": applied_preview,
        }),
    );
    Some(())
}

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
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS import_learning_samples (
                id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
                user_id BIGINT NOT NULL,
                sample_key TEXT NOT NULL DEFAULT '',
                normalized_features JSONB NOT NULL DEFAULT '{}'::jsonb,
                target_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
                source_payload JSONB NOT NULL DEFAULT '{}'::jsonb
            )
            "#,
        )
        .execute(pool)
        .await?;
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS import_learning_features (
                id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
                user_id BIGINT NOT NULL,
                sample_id BIGINT NOT NULL,
                feature_key TEXT NOT NULL,
                feature_hash TEXT NOT NULL,
                feature_payload JSONB NOT NULL DEFAULT '{}'::jsonb
            )
            "#,
        )
        .execute(pool)
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
}
