/// 在独立单线程 runtime 中执行 Weaviate recall，限制并发并把网络失败降级为状态文本。
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
                                scope_order: request.scope_order,
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
                results.sort_by_key(|result| (result.draft_index, result.scope_order));
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
