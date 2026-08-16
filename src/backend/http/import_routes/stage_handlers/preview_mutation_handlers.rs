/// 处理单行 preview 更新，构建 patch、落库身份校验，并按 responseMode 返回单行或汇总。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_preview_update_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "import_preview_update_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let preview_id = match preview_id_from_payload(object) {
        Ok(preview_id) => preview_id,
        Err(response) => return route_response(response),
    };
    let preview = match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
        Ok(Some(preview)) if preview.session_id == session_id => preview,
        Ok(Some(_)) | Ok(None) => {
            return route_response(import_v2_error_response(404, "Preview bill not found"));
        }
        Err(error) => return route_response(db_error_response(error)),
    };
    let patch = match build_preview_patch_from_payload_with_category_lookup(
        runtime.connection(),
        user_id,
        preview.id,
        object,
    ) {
        Ok(patch) => patch,
        Err(response) => return route_response(response),
    };
    let updated = match update_preview_bill(runtime.connection(), &session_id, user_id, &patch) {
        Ok(updated) => updated,
        Err(error) => return route_response(db_error_response(error)),
    };
    if response_mode_is_preview_item(object) {
        let preview_item = match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
            Ok(Some(preview)) if preview.session_id == session_id => preview_row_to_value(preview),
            Ok(Some(_)) | Ok(None) => Value::Null,
            Err(error) => return route_response(db_error_response(error)),
        };
        route_response(import_v2_data_response(json!({
            "updated": updated,
            "previewItem": preview_item,
        })))
    } else {
        route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": updated}),
        })
    }
}

/// 处理批量 reclassify 请求，复用 preview update patch 链路并刷新分类/账户匹配统计。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_reclassify_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "import_reclassify_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }
    let update_items = match preview_update_items_from_payload(&payload) {
        Ok(update_items) => update_items,
        Err(response) => return route_response(response),
    };
    let mut patches = Vec::with_capacity(update_items.len());
    let mut target_ids = Vec::with_capacity(update_items.len());
    for item in update_items {
        let preview_id = match preview_id_from_payload(item) {
            Ok(preview_id) => preview_id,
            Err(response) => return route_response(response),
        };
        match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
            Ok(Some(preview)) if preview.session_id == session_id => {}
            Ok(Some(_)) | Ok(None) => {
                return route_response(import_v2_error_response(404, "Preview bill not found"));
            }
            Err(error) => return route_response(db_error_response(error)),
        }
        patches.push(match build_preview_patch_from_payload_with_category_lookup(
            runtime.connection(),
            user_id,
            preview_id,
            item,
        ) {
            Ok(patch) => patch,
            Err(response) => return route_response(response),
        });
        target_ids.push(preview_id);
    }
    target_ids.sort_unstable();
    target_ids.dedup();
    let updated = match update_preview_bills_batch(
        runtime.connection_mut(),
        &session_id,
        user_id,
        patches.as_slice(),
    ) {
        Ok(updated) => updated,
        Err(error) => return route_response(db_error_response(error)),
    };
    let preview = if target_ids.is_empty() {
        get_preview_by_session(runtime.connection(), &session_id, user_id, false)
    } else {
        get_preview_by_ids(runtime.connection(), &session_id, &target_ids, user_id)
    };
    let mut preview = match preview {
        Ok(preview) => preview,
        Err(error) => return route_response(db_error_response(error)),
    };
    let mut intelligent_drafts = preview
        .iter()
        .map(import_preview_draft_from_row)
        .collect::<Vec<_>>();
    for draft in &mut intelligent_drafts {
        invalidate_reclassification_dependent_signals(draft);
    }
    if let Err(error) = ImportStage2::evaluate(
        runtime.connection_mut(),
        user_id,
        intelligent_drafts.as_mut_slice(),
    )
    .await
    {
        return route_response(db_error_response(error));
    }
    enforce_import_preview_invariants(intelligent_drafts.as_mut_slice());
    let intelligence_patches = preview
        .iter()
        .zip(intelligent_drafts.iter())
        .map(|(row, draft)| import_preview_patch_from_draft(row.id, draft))
        .collect::<Vec<_>>();
    if let Err(error) = update_preview_bills_batch(
        runtime.connection_mut(),
        &session_id,
        user_id,
        intelligence_patches.as_slice(),
    ) {
        return route_response(db_error_response(error));
    }
    preview = match if target_ids.is_empty() {
        get_preview_by_session(runtime.connection(), &session_id, user_id, false)
    } else {
        get_preview_by_ids(runtime.connection(), &session_id, &target_ids, user_id)
    } {
        Ok(preview) => preview,
        Err(error) => return route_response(db_error_response(error)),
    };
    let categorized = preview.iter().filter(preview_row_is_categorized).count();
    let account_matched = preview.iter().filter(preview_row_has_account).count();
    let preview: Vec<Value> = preview.into_iter().map(preview_row_to_value).collect();
    route_response(import_v2_data_response(json!({
        "session_id": session_id,
        "total": preview.len(),
        "categorized": categorized,
        "account_matched": account_matched,
        "session_samples_saved": 0,
        "annotation_applied": 0,
        "updated": updated,
        "preview": preview,
    })))
}

/// Reclassification changes the semantic inputs consumed by recommendation and history matching.
/// Drop those projections before rebuilding the intelligence chain so the response cannot expose
/// an accepted/rejected recommendation or destructive history plan for the previous row state.
fn invalidate_reclassification_dependent_signals(draft: &mut ImportPreviewDraft) {
    draft.preview_recurring_id = None;
    draft.preview_recurring_name.clear();
    draft.preview_recurring_candidate_count = 0;
    draft.preview_recurring_match_score = 0.0;
    draft.preview_recurring_match_reasons.clear();
    draft.preview_recurring_matched_date.clear();

    let Some(feedback) = draft.preview_matching_feedback.as_object_mut() else {
        draft.preview_matching_feedback = json!({});
        return;
    };

    feedback.remove("learning");
    feedback.remove("llm");
    feedback.remove("history");
    feedback.remove("recurring");

    let reconciliation_depends_on_history = feedback
        .get("reconciliation")
        .and_then(Value::as_object)
        .is_some_and(|reconciliation| {
            [
                "planned_operation",
                "history_bill_id",
                "history_bill_version",
                "operation_id",
                "acknowledgement_token",
                "destructive_ack_required",
            ]
            .iter()
            .any(|key| reconciliation.contains_key(*key))
        });
    if reconciliation_depends_on_history {
        feedback.remove("reconciliation");
    }
}

async fn reclassify_dematerialized_preview_items(
    runtime: &mut ImportRuntime,
    session_id: &str,
    user_id: UserId,
    group_id: i64,
    operation_id: &str,
    preview_ids: &[i64],
) -> Result<Vec<Value>, bill_analyser_db::DbError> {
    let Some(owner_token) =
        claim_import_group_reclassification(runtime.connection(), user_id, group_id)?
    else {
        let (status, items) = get_import_group_reclassification_state(
            runtime.connection(),
            user_id,
            group_id,
            operation_id,
        )?;
        return match status.as_str() {
            "completed" if !items.is_empty() => Ok(items),
            "completed" => Err(bill_analyser_db::DbError::InvalidOperation(
                "decision group reclassification completed without preview items".into(),
            )),
            "running" | "pending" => Err(bill_analyser_db::DbError::InvalidOperation(
                "decision group reclassification pending".into(),
            )),
            "failed" => Err(bill_analyser_db::DbError::InvalidOperation(
                "decision group reclassification retry conflict".into(),
            )),
            _ => Err(bill_analyser_db::DbError::InvalidOperation(
                "decision group reclassification state missing".into(),
            )),
        };
    };
    let result =
        reclassify_dematerialized_preview_items_claimed(runtime, session_id, user_id, preview_ids)
            .await;
    match &result {
        Ok(items) => finish_import_group_reclassification(
            runtime.connection(),
            user_id,
            group_id,
            operation_id,
            &owner_token,
            "completed",
            None,
            items,
        )?,
        Err(error) => finish_import_group_reclassification(
            runtime.connection(),
            user_id,
            group_id,
            operation_id,
            &owner_token,
            "failed",
            Some(&error.to_string()),
            &[],
        )?,
    }
    result
}

async fn reclassify_dematerialized_preview_items_claimed(
    runtime: &mut ImportRuntime,
    session_id: &str,
    user_id: UserId,
    preview_ids: &[i64],
) -> Result<Vec<Value>, bill_analyser_db::DbError> {
    let preview = get_preview_by_session(runtime.connection(), session_id, user_id, false)?
        .into_iter()
        .filter(|row| preview_ids.contains(&row.id))
        .collect::<Vec<_>>();
    if preview.is_empty() {
        return Err(bill_analyser_db::DbError::InvalidOperation(
            "decision group reclassification has no preview rows".into(),
        ));
    }
    let mut drafts = preview
        .iter()
        .map(import_preview_draft_from_row)
        .collect::<Vec<_>>();
    ImportStage2::evaluate(runtime.connection_mut(), user_id, drafts.as_mut_slice())
        .await?;
    enforce_import_preview_invariants(drafts.as_mut_slice());
    let patches = preview
        .iter()
        .zip(drafts.iter())
        .map(|(row, draft)| import_preview_patch_from_draft(row.id, draft))
        .collect::<Vec<_>>();
    update_preview_bills_batch(runtime.connection_mut(), session_id, user_id, &patches)?;
    let refreshed = get_preview_by_session(runtime.connection(), session_id, user_id, false)?;
    let items = refreshed
        .into_iter()
        .filter(|row| preview_ids.contains(&row.id))
        .map(preview_row_to_value)
        .collect::<Vec<_>>();
    if items.is_empty() {
        return Err(bill_analyser_db::DbError::InvalidOperation(
            "decision group reclassification produced no preview items".into(),
        ));
    }
    Ok(items)
}
