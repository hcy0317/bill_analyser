/// 执行导入 stage2 主编排：dedup、历史 materialization、规则/learning/recall、preview 写入与异步 decision group 物化。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_dedup_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "import_dedup_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let session_id = match required_session_id_from_payload(object) {
        Ok(session_id) => session_id,
        Err(response) => return route_response(response),
    };
    let include_preview =
        bool_field_from_object(object, &["include_preview", "includePreview"]).unwrap_or(false);
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    let session = match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(session)) => session,
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    };

    let _stage_started_at = Instant::now();
    let _template_query_started_at = Instant::now();
    let templates =
        match get_unprocessed_templates_for_dedup(runtime.connection(), &session_id, user_id) {
            Ok(templates) => templates,
            Err(error) => return route_response(db_error_response(error)),
        };
    if templates.is_empty() && session.status == "preview" {
        let persisted_preview =
            match get_preview_by_session(runtime.connection(), &session_id, user_id, false) {
                Ok(rows) => rows,
                Err(error) => return route_response(db_error_response(error)),
            };
        let persisted_preview_count = persisted_preview.len();
        let preview = if include_preview {
            persisted_preview
                .into_iter()
                .map(preview_row_to_value)
                .collect()
        } else {
            Vec::new()
        };
        let total = usize::try_from(session.total_parsed.max(0)).unwrap_or(usize::MAX);
        return route_response(import_stage_dedup_success(ImportStageDedupData {
            session_id,
            preview,
            preview_included: include_preview,
            total,
            after_dedup: persisted_preview_count,
            dedup_stats: json!({
                "removed": total.saturating_sub(persisted_preview_count),
                "duplicates": 0,
                "transfer_pairs": 0,
                "split_merge": 0,
            }),
            match_stats: json!({
                "runtime": "rust",
                "category_matched": 0,
                "account_matched": 0,
                "learning_applied": 0,
                "recurring_projected": 0,
                "database_candidates": 0,
                "provider_bypassed": false,
                "idempotent_replay": true,
            }),
        }));
    }
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_dedup_runtime_handler",
        user_id = user_id.get(),
        session_id = %session_id,
        templates = templates.len(),
        elapsed_ms = import_stage_elapsed_ms(_template_query_started_at),
        "stage2 templates loaded"
    );
    let _dedup_started_at = Instant::now();
    let dedup_input = dedup_bills_from_parser_templates(&templates);
    let dedup_result = SmartDeduplicationEngine.process(dedup_input);
    let standard_rows =
        match get_import_standard_rows_by_session(runtime.connection(), &session_id, user_id) {
            Ok(rows) => rows,
            Err(error) => return route_response(db_error_response(error)),
        };
    let history_bills = match get_import_history_candidate_bills_for_session(
        runtime.connection(),
        &session_id,
        user_id,
    ) {
        Ok(rows) => rows,
        Err(error) => return route_response(db_error_response(error)),
    };
    let mut history_duplicate_plan =
        build_history_duplicate_preview_plan(&dedup_result.kept_bills, &history_bills);
    let history_duplicate_import_keys = history_duplicate_plan
        .iter()
        .map(|plan| plan.import_bill_key.clone())
        .collect::<HashSet<_>>();
    let history_duplicate_ids = history_duplicate_plan
        .iter()
        .map(|plan| plan.history_bill_id)
        .collect::<HashSet<_>>();
    let mut history_transfer_plan = build_history_transfer_preview_plan(
        &dedup_result.kept_bills,
        &history_bills,
        &history_duplicate_import_keys,
        &history_duplicate_ids,
    );
    let _dedup_elapsed_ms = import_stage_elapsed_ms(_dedup_started_at);
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_dedup_runtime_handler",
        user_id = user_id.get(),
        session_id = %session_id,
        original = dedup_result.original_count,
        kept = dedup_result.kept_bills.len(),
        removed = dedup_result.removed_count,
        elapsed_ms = _dedup_elapsed_ms,
        "stage2 smart dedup complete"
    );
    let history_import_keys = history_duplicate_plan
        .iter()
        .map(|plan| plan.import_bill_key.clone())
        .chain(
            history_transfer_plan
                .iter()
                .map(|plan| plan.import_bill_key.clone()),
        )
        .collect::<HashSet<_>>();
    let previewable_bills = dedup_result
        .kept_bills
        .iter()
        .filter(|bill| !history_import_keys.contains(&import_reconciliation_key_for_bill(bill)))
        .cloned()
        .collect::<Vec<_>>();
    let mut preview_drafts = preview_drafts_from_dedup_bills(&previewable_bills);
    preview_drafts.extend(
        history_duplicate_plan
            .iter()
            .map(|plan| plan.preview_draft.clone()),
    );
    preview_drafts.extend(
        history_transfer_plan
            .iter()
            .map(|plan| plan.preview_draft.clone()),
    );
    let _intelligence_started_at = Instant::now();
    let mut intelligence_stats = match apply_import_intelligence_chain(
        runtime.connection_mut(),
        user_id,
        preview_drafts.as_mut_slice(),
    )
    .await
    {
        Ok(stats) => stats,
        Err(error) => return route_response(db_error_response(error)),
    };
    let _intelligence_elapsed_ms = import_stage_elapsed_ms(_intelligence_started_at);
    let user_id_i64 = match user_id_i64_for_sql(user_id) {
        Ok(user_id) => user_id,
        Err(error) => return route_response(db_error_response(error)),
    };
    let _vector_recall_started_at = Instant::now();
    match apply_import_learning_vector_recall_chain(
        runtime.connection(),
        &state.config,
        user_id_i64,
        preview_drafts.as_mut_slice(),
    )
    .await
    {
        Ok(vector_stats) => intelligence_stats.merge_vector_recall(vector_stats),
        Err(error) => return route_response(db_error_response(error)),
    }
    let _vector_recall_elapsed_ms = import_stage_elapsed_ms(_vector_recall_started_at);
    enforce_import_preview_invariants(preview_drafts.as_mut_slice());
    let _category_missing_count = preview_drafts
        .iter()
        .filter(|draft| {
            draft.category_id.is_none()
                && draft.preview_main_category.trim().is_empty()
                && draft.preview_sub_category.trim().is_empty()
        })
        .count();
    let _transfer_candidate_count = preview_drafts
        .iter()
        .filter(|draft| is_transfer_protected_preview(draft))
        .count();
    let _learning_candidate_count = preview_drafts
        .iter()
        .filter(|draft| draft.preview_matching_feedback.get("learning").is_some())
        .count();
    refresh_history_duplicate_materialization_payloads(
        &mut history_duplicate_plan,
        &preview_drafts,
    );
    refresh_history_transfer_materialization_payloads(&mut history_transfer_plan, &preview_drafts);
    let _preview_insert_started_at = Instant::now();
    if !templates.is_empty() {
        if let Err(error) =
            clear_import_preview_materialization_state(runtime.connection_mut(), &session_id, user_id)
        {
            return route_response(db_error_response(error));
        }
    }
    let inserted_preview = match insert_preview_bills_batch(
        runtime.connection_mut(),
        &session_id,
        user_id,
        &preview_drafts,
    ) {
        Ok(inserted) => inserted,
        Err(error) => return route_response(db_error_response(error)),
    };
    if let Err(error) = insert_import_history_materializations_batch(
        runtime.connection_mut(),
        &session_id,
        user_id,
        &history_duplicate_plan
            .iter()
            .map(|plan| plan.materialization.clone())
            .chain(
                history_transfer_plan
                    .iter()
                    .map(|plan| plan.materialization.clone()),
            )
            .collect::<Vec<_>>(),
    ) {
        return route_response(db_error_response(error));
    }
    let _preview_insert_elapsed_ms = import_stage_elapsed_ms(_preview_insert_started_at);
    let database_candidate_count = history_duplicate_plan.len() + history_transfer_plan.len();
    if let Err(error) = set_import_decision_materialization_status(
        runtime.connection(),
        &session_id,
        user_id,
        "pending",
    ) {
        tracing::error!(operation = "import_decision_group_materialization", error = %error, "failed to mark decision materialization pending");
        return route_response(import_v2_error_response(500, "Failed to mark decision materialization pending"));
    }
    spawn_import_decision_group_materialization(ImportDecisionGroupMaterializationInput {
        pool: runtime.pool().clone(),
        session_id: session_id.clone(),
        user_id,
        duplicate_groups: dedup_result.duplicate_groups.clone(),
        transfer_pairs: dedup_result.transfer_pairs.clone(),
        templates: templates.clone(),
        standard_rows,
        history_duplicate_plan,
        history_transfer_plan,
    });
    let _decision_groups_elapsed_ms = 0u128;
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_dedup_runtime_handler",
        user_id = user_id.get(),
        session_id = %session_id,
        preview_rows = inserted_preview,
        elapsed_ms = _preview_insert_elapsed_ms,
        decision_groups_deferred = true,
        decision_groups_elapsed_ms = _decision_groups_elapsed_ms,
        "stage2 preview inserted"
    );
    let _status_update_started_at = Instant::now();
    if let Err(error) = update_import_session_status(
        runtime.connection(),
        &ImportSessionStatusUpdate {
            session_id: session_id.clone(),
            user_id,
            status: "preview".to_string(),
            total_parsed: Some(usize_to_i64(templates.len())),
            total_preview: Some(usize_to_i64(inserted_preview)),
            total_confirmed: None,
        },
    ) {
        return route_response(db_error_response(error));
    }
    let _updated_templates = match mark_unprocessed_parser_templates_processed_for_session(
        runtime.connection_mut(),
        &session_id,
        user_id,
    ) {
        Ok(updated) => updated,
        Err(error) => return route_response(db_error_response(error)),
    };
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_dedup_runtime_handler",
        user_id = user_id.get(),
        session_id = %session_id,
        template_rows = templates.len(),
        updated_template_rows = _updated_templates,
        total_elapsed_ms = import_stage_elapsed_ms(_stage_started_at),
        status_elapsed_ms = import_stage_elapsed_ms(_status_update_started_at),
        "stage2 status updated"
    );
    let _api_response_started_at = Instant::now();
    let preview = if include_preview {
        match get_preview_by_session(runtime.connection(), &session_id, user_id, false) {
            Ok(rows) => rows.into_iter().map(preview_row_to_value).collect(),
            Err(error) => return route_response(db_error_response(error)),
        }
    } else {
        Vec::new()
    };
    let _api_response_elapsed_ms = import_stage_elapsed_ms(_api_response_started_at);
    let _total_elapsed_ms = import_stage_elapsed_ms(_stage_started_at);
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "import_parser",
        operation = "import_dedup_runtime_handler",
        user_id = user_id.get(),
        session_id = %session_id,
        file_count = templates.len(),
        raw_transaction_count = dedup_result.original_count,
        preview_count = inserted_preview,
        dedup_group_count = dedup_result.duplicate_groups.len()
            + dedup_result.transfer_pairs.len()
            + dedup_result.split_groups.len(),
        category_missing_count = _category_missing_count,
        transfer_candidate_count = _transfer_candidate_count,
        learning_candidate_count = _learning_candidate_count,
        elapsed_total_ms = _total_elapsed_ms,
        elapsed_parse_ms = 0u128,
        elapsed_dedup_ms = _dedup_elapsed_ms,
        elapsed_intelligence_ms = _intelligence_elapsed_ms,
        elapsed_learning_ms = _vector_recall_elapsed_ms,
        elapsed_identity_validation_ms = 0u128,
        elapsed_preview_insert_ms = _preview_insert_elapsed_ms,
        elapsed_decision_groups_ms = _decision_groups_elapsed_ms,
        decision_groups_deferred = true,
        elapsed_api_response_ms = _api_response_elapsed_ms,
        elapsed_intelligence_load_ms = intelligence_stats._elapsed_load_ms,
        elapsed_category_rule_ms = intelligence_stats.elapsed_category_rule_ns / 1_000_000,
        elapsed_recurring_rule_ms = intelligence_stats.elapsed_recurring_rule_ns / 1_000_000,
        elapsed_learning_rule_ms = intelligence_stats.elapsed_learning_rule_ns / 1_000_000,
        elapsed_account_rule_ms = intelligence_stats.elapsed_account_rule_ns / 1_000_000,
        elapsed_stage2_baseline_ms = intelligence_stats.elapsed_stage2_baseline_ns / 1_000_000,
        "import stage2 summary"
    );
    route_response(import_stage_dedup_success(ImportStageDedupData {
        session_id,
        preview,
        preview_included: include_preview,
        total: dedup_result.original_count,
        after_dedup: inserted_preview,
        dedup_stats: json!({
            "removed": dedup_result.removed_count,
            "duplicates": dedup_result.duplicate_groups.len(),
            "transfer_pairs": dedup_result.transfer_pairs.len(),
            "split_merge": dedup_result.split_groups.len(),
        }),
        match_stats: json!({
            "runtime": "rust",
            "category_matched": intelligence_stats.category_matched,
            "account_matched": intelligence_stats.account_matched,
            "learning_applied": intelligence_stats.learning_applied,
            "learning_vector_recalled": intelligence_stats.learning_vector_recalled,
            "learning_vector_status": intelligence_stats.learning_vector_status,
            "recurring_projected": intelligence_stats.recurring_projected,
            "database_candidates": database_candidate_count,
            "provider_bypassed": false,
        }),
    }))
}
