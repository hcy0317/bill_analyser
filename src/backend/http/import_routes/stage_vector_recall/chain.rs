/// 执行导入 learning 向量召回链路；无向量源时必须跳过网络请求并保持 preview 可用。
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

/// 为未命中 learning 的 preview draft 构建 Weaviate recall 请求，非转账只查询收入/支出/投资 scope。
fn build_import_learning_vector_recall_requests(
    config: &HttpShellConfig,
    user_id: i64,
    drafts: &[ImportPreviewDraft],
) -> Vec<ImportLearningVectorRecallRequestDraft> {
    drafts
        .iter()
        .enumerate()
        .filter(|(_, draft)| draft.preview_matching_feedback.get("learning").is_none())
        .flat_map(|(draft_index, draft)| {
            let features = build_composite_match_features(
                &draft.preview_parser_id,
                &draft.preview_counterparty,
                &draft.preview_description,
                &draft.preview_payment_method,
            );
            let Some(features) = features else {
                return Vec::new();
            };
            import_learning_vector_recall_scopes(draft)
                .into_iter()
                .enumerate()
                .filter_map(|(scope_order, transaction_type_scope)| {
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
                        scope_order,
                        features: features.clone(),
                        transaction_type_scope,
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn import_learning_vector_recall_scopes(draft: &ImportPreviewDraft) -> Vec<String> {
    if is_transfer_protected_preview(draft) {
        return vec!["transfer".to_string()];
    }
    vec![
        normalize_weaviate_transaction_type_scope("income"),
        normalize_weaviate_transaction_type_scope("expense"),
        normalize_weaviate_transaction_type_scope("investment"),
    ]
}
