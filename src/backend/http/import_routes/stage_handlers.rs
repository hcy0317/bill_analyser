pub async fn import_dedup_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
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
        bool_field_from_object(object, &["include_preview", "includePreview"]).unwrap_or(true);
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

    let stage_started_at = Instant::now();
    let template_query_started_at = Instant::now();
    let templates =
        match get_unprocessed_templates_for_dedup(runtime.connection(), &session_id, user_id) {
            Ok(templates) => templates,
            Err(error) => return route_response(db_error_response(error)),
        };
    eprintln!(
        "[bill analyser import] stage2 templates loaded user_id={} session_id={} templates={} elapsed_ms={}",
        user_id.get(),
        session_id,
        templates.len(),
        import_stage_elapsed_ms(template_query_started_at)
    );
    let dedup_started_at = Instant::now();
    let dedup_input = dedup_bills_from_parser_templates(&templates);
    let dedup_result = SmartDeduplicationEngine.process(dedup_input);
    eprintln!(
        "[bill analyser import] stage2 smart dedup complete user_id={} session_id={} original={} kept={} removed={} dedup_elapsed_ms={}",
        user_id.get(),
        session_id,
        dedup_result.original_count,
        dedup_result.kept_bills.len(),
        dedup_result.removed_count,
        import_stage_elapsed_ms(dedup_started_at)
    );
    let preview_drafts = preview_drafts_from_dedup_bills(&dedup_result.kept_bills);
    let preview_insert_started_at = Instant::now();
    let inserted_preview = match insert_preview_bills_batch(
        runtime.connection_mut(),
        &session_id,
        user_id,
        &preview_drafts,
    ) {
        Ok(inserted) => inserted,
        Err(error) => return route_response(db_error_response(error)),
    };
    eprintln!(
        "[bill analyser import] stage2 preview inserted user_id={} session_id={} preview_rows={} insert_elapsed_ms={}",
        user_id.get(),
        session_id,
        inserted_preview,
        import_stage_elapsed_ms(preview_insert_started_at)
    );
    let template_ids = templates
        .iter()
        .map(|template| template.id)
        .collect::<Vec<_>>();
    let status_update_started_at = Instant::now();
    if let Err(error) =
        update_parser_template_status(runtime.connection_mut(), &template_ids, true, None, user_id)
    {
        return route_response(db_error_response(error));
    }
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
    eprintln!(
        "[bill analyser import] stage2 status updated user_id={} session_id={} template_rows={} total_elapsed_ms={} status_elapsed_ms={}",
        user_id.get(),
        session_id,
        template_ids.len(),
        import_stage_elapsed_ms(stage_started_at),
        import_stage_elapsed_ms(status_update_started_at)
    );
    let preview = if include_preview {
        match get_preview_by_session(runtime.connection(), &session_id, user_id, false) {
            Ok(rows) => rows.into_iter().map(preview_row_to_value).collect(),
            Err(error) => return route_response(db_error_response(error)),
        }
    } else {
        Vec::new()
    };
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
            "database_candidates": 0,
            "provider_bypassed": true,
        }),
    }))
}

pub async fn import_confirm_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
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

    if first_value(object, &["preview_updates", "previewUpdates"]).is_some() {
        let update_items = match preview_update_items_from_payload(&payload) {
            Ok(items) => items,
            Err(response) => return route_response(response),
        };
        let mut patches = Vec::with_capacity(update_items.len());
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
            patches.push(build_preview_patch_from_payload(preview_id, item));
        }
        if let Err(error) = replace_preview_selection_with_patches(
            runtime.connection_mut(),
            &session_id,
            user_id,
            &patches,
        ) {
            return route_response(db_error_response(error));
        }
    } else if let Some(selected_ids) =
        id_list_field_from_object(object, &["selected_ids", "selectedIds"])
    {
        for preview_id in &selected_ids {
            match get_preview_bill_by_id(runtime.connection(), *preview_id, user_id) {
                Ok(Some(preview)) if preview.session_id == session_id => {}
                Ok(Some(_)) | Ok(None) => {
                    return route_response(import_v2_error_response(404, "Preview bill not found"));
                }
                Err(error) => return route_response(db_error_response(error)),
            }
        }
        if let Err(error) =
            reset_session_preview_selection(runtime.connection(), &session_id, user_id)
        {
            return route_response(db_error_response(error));
        }
        if let Err(error) =
            update_preview_selection(runtime.connection_mut(), &selected_ids, true, user_id)
        {
            return route_response(db_error_response(error));
        }
    }

    let result = match confirm_preview_to_bills(runtime.connection_mut(), &session_id, user_id) {
        Ok(result) => result,
        Err(error) => return route_response(db_error_response(error)),
    };
    route_response(import_stage_confirm_success(ImportStageConfirmData {
        imported_count: result.confirmed_count,
        skipped_count: result.skipped_count + result.duplicate_count,
        errors: result.errors,
    }))
}

pub async fn import_session_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
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
        Ok(Some(session)) => route_response(import_session_success(ImportSessionSummary {
            session_id: session.session_id,
            status: session.status,
            created_at: session.created_at,
            parsed_count: non_negative_usize(session.total_parsed),
            preview_count: non_negative_usize(session.total_preview),
            file_paths: json!([]),
        })),
        Ok(None) => route_response(import_session_not_found_response()),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn import_session_cancel_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
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
        Ok(Some(_)) => match clear_session_data(runtime.connection_mut(), &session_id, user_id) {
            Ok(_) => route_response(import_session_cancel_success_response()),
            Err(error) => route_response(db_error_response(error)),
        },
        Ok(None) => route_response(import_session_cancel_missing_response()),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn import_preview_page_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    Query(query): Query<PreviewPageQuery>,
    headers: HeaderMap,
) -> Response {
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
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query
        .page_size
        .or(query.page_size_camel)
        .unwrap_or(100)
        .clamp(1, 500);
    let selected_only = query
        .selected_only
        .or(query.selected_only_camel)
        .unwrap_or(false);
    match get_preview_page_by_session(
        runtime.connection(),
        &session_id,
        user_id,
        page as i64,
        page_size as i64,
        selected_only,
    ) {
        Ok((preview, total)) => {
            let preview = preview
                .into_iter()
                .map(|row| serde_json::to_value(row).unwrap_or_else(|_| json!({})))
                .collect();
            route_response(import_preview_page_success(ImportPreviewPageData {
                preview,
                total: non_negative_usize(total),
                page,
                page_size,
            }))
        }
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn import_preview_index_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
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
    let rows = match get_preview_by_session(runtime.connection(), &session_id, user_id, false) {
        Ok(rows) => rows,
        Err(error) => return route_response(db_error_response(error)),
    };
    let categories_by_id = BTreeMap::new();
    let accounts_by_id = BTreeMap::new();
    let items = rows
        .into_iter()
        .filter_map(|row| preview_row_to_value(row).as_object().cloned())
        .map(|row| build_import_preview_filter_index_item(&row, &categories_by_id, &accounts_by_id))
        .collect::<Vec<_>>();
    route_response(import_preview_index_success(ImportPreviewIndexData {
        total: items.len(),
        items,
    }))
}

pub async fn import_preview_update_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
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
    let patch = build_preview_patch_from_payload(preview.id, object);
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

pub async fn import_reclassify_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
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
        patches.push(build_preview_patch_from_payload(preview_id, item));
    }
    let updated = match update_preview_bills_batch(
        runtime.connection_mut(),
        &session_id,
        user_id,
        patches.as_slice(),
    ) {
        Ok(updated) => updated,
        Err(error) => return route_response(db_error_response(error)),
    };
    let preview = match get_preview_by_session(runtime.connection(), &session_id, user_id, false) {
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

pub async fn preview_recurring_candidates_runtime_handler(
    State(state): State<HttpAppState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
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
    let preview = match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
        Ok(Some(preview)) => preview,
        Ok(None) => return route_response(import_v2_error_response(404, "Preview bill not found")),
        Err(error) => return route_response(db_error_response(error)),
    };
    route_response(import_v2_data_response(json!({
        "previewId": preview.id,
        "sessionId": preview.session_id,
        "linkedRecurringId": preview.preview_recurring_id,
        "linkedRecurringName": preview.preview_recurring_name,
        "candidates": [],
        "candidate_count": 0,
        "provider_bypassed": true,
        "runtime": "rust-import-db-runtime-partial",
    })))
}

pub async fn preview_recurring_match_put_runtime_handler(
    State(state): State<HttpAppState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let recurring_id = match first_value(object, &["recurringId", "recurring_id"])
        .and_then(value_to_i64)
        .filter(|value| *value > 0)
    {
        Some(value) => value,
        None => return route_response(import_v2_error_response(400, "Missing recurringId")),
    };
    let expected_state = match expected_state_from_payload(object) {
        Ok(expected_state) => expected_state,
        Err(response) => return route_response(response),
    };
    let target_candidate = recurring_candidate_from_payload(object, recurring_id);
    let update = ImportPreviewRecurringMatchUpdate {
        recurring_id: Some(recurring_id),
        candidate_count: recurring_candidate_count_from_payload(object, target_candidate.as_ref()),
        target_candidate,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match update_preview_recurring_match_decision(
        runtime.connection_mut(),
        preview_id,
        user_id,
        &update,
        Some(&expected_state),
    ) {
        Ok(result) => route_response(preview_decision_result_response(
            result,
            json!({"recurringId": recurring_id}),
        )),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn preview_recurring_match_delete_runtime_handler(
    State(state): State<HttpAppState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let expected_state = match expected_state_from_payload(object) {
        Ok(expected_state) => expected_state,
        Err(response) => return route_response(response),
    };
    let update = ImportPreviewRecurringMatchUpdate {
        recurring_id: None,
        candidate_count: 0,
        target_candidate: None,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match update_preview_recurring_match_decision(
        runtime.connection_mut(),
        preview_id,
        user_id,
        &update,
        Some(&expected_state),
    ) {
        Ok(result) => route_response(preview_decision_result_response(
            result,
            json!({"recurringId": Value::Null}),
        )),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn preview_transfer_decision_runtime_handler(
    State(state): State<HttpAppState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let decision = match decision_from_payload(object) {
        Ok(decision) => decision,
        Err(response) => return route_response(response),
    };
    let expected_state = match expected_state_from_payload(object) {
        Ok(expected_state) => expected_state,
        Err(response) => return route_response(response),
    };
    let reviewed_type = first_value(object, &["reviewedType", "reviewed_type"])
        .and_then(value_to_text)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "转账".to_string());
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match apply_preview_transfer_decision(
        runtime.connection_mut(),
        preview_id,
        user_id,
        decision,
        &reviewed_type,
        Some(&expected_state),
    ) {
        Ok(result) => route_response(preview_decision_result_response(
            result,
            json!({"decision": decision_name(decision)}),
        )),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn import_learning_suggestions_get_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    import_learning_suggestions_response(state, session_id, headers, None).await
}

pub async fn import_learning_suggestions_post_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    import_learning_suggestions_response(state, session_id, headers, Some(payload)).await
}

async fn import_learning_suggestions_response(
    state: HttpAppState,
    session_id: String,
    headers: HeaderMap,
    payload: Option<Value>,
) -> Response {
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
    let applied_preview_updates = match payload.as_ref() {
        Some(payload) => {
            match apply_preview_updates_from_payload(&mut runtime, &session_id, user_id, payload) {
                Ok(updated) => updated,
                Err(response) => return route_response(response),
            }
        }
        None => 0,
    };
    let preview_ids = payload
        .as_ref()
        .map(preview_ids_from_payload)
        .unwrap_or_default();
    route_response(import_v2_data_response(json!({
        "session_id": session_id,
        "suggestions": [],
        "count": 0,
        "preview_ids": preview_ids,
        "applied_preview_updates": applied_preview_updates,
        "provider_bypassed": true,
        "runtime": "rust-import-db-runtime-partial",
    })))
}

pub async fn import_learning_promote_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }
    let applied_preview_updates =
        match apply_preview_updates_from_payload(&mut runtime, &session_id, user_id, &payload) {
            Ok(updated) => updated,
            Err(response) => return route_response(response),
        };
    let annotation_samples = annotation_samples_from_payload(&payload);
    let saved_samples = if annotation_samples.is_empty() {
        0
    } else {
        match save_import_annotation_samples(
            runtime.connection_mut(),
            &session_id,
            user_id,
            &annotation_samples,
        ) {
            Ok(saved) => saved,
            Err(error) => return route_response(db_error_response(error)),
        }
    };
    let mut preview_ids = preview_ids_from_payload(&payload);
    preview_ids.extend(annotation_samples.iter().map(|sample| sample.preview_id));
    preview_ids.sort_unstable();
    preview_ids.dedup();
    let promoted = match promote_import_learning_rules(
        runtime.connection_mut(),
        &session_id,
        user_id,
        &preview_ids,
    ) {
        Ok(result) => result,
        Err(response) => return route_response(response),
    };
    route_response(import_v2_data_response(json!({
        "success": true,
        "session_id": session_id,
        "selected_samples": preview_ids.len(),
        "saved_samples": saved_samples,
        "rules_total": promoted.rules_total,
        "created": promoted.created,
        "updated": promoted.updated,
        "applied_preview_updates": applied_preview_updates,
        "provider_bypassed": true,
        "runtime": "rust-import-db-runtime-partial",
    })))
}

pub async fn import_learning_rules_list_runtime_handler(
    State(state): State<HttpAppState>,
    Query(query): Query<ImportLearningRulesQuery>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size().clamp(1, 500);
    let enabled_only = query.enabled_only();
    let total = match count_import_learning_rules(runtime.connection(), user_id, enabled_only) {
        Ok(total) => total,
        Err(response) => return route_response(response),
    };
    let offset = (page.saturating_sub(1)).saturating_mul(page_size);
    let rules = match load_import_learning_rules(
        runtime.connection(),
        user_id,
        enabled_only,
        page_size,
        offset,
    ) {
        Ok(rules) => rules,
        Err(response) => return route_response(response),
    };
    let total_pages = if total <= 0 {
        0
    } else {
        (total as usize).div_ceil(page_size) as i64
    };
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "result": rules,
            "totalCount": total,
            "page": page,
            "pageSize": page_size,
            "totalPages": total_pages,
        }),
    })
}

pub async fn import_learning_rule_update_runtime_handler(
    State(state): State<HttpAppState>,
    Path(rule_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    if let Err(response) =
        update_import_learning_rule(runtime.connection(), rule_id, user_id, object)
    {
        return route_response(response);
    }
    match get_import_learning_rule(runtime.connection(), rule_id, user_id) {
        Ok(Some(rule)) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": true, "result": rule}),
        }),
        Ok(None) => route_response(import_v2_error_response(404, "Rule not found")),
        Err(response) => route_response(response),
    }
}

pub async fn import_learning_rule_delete_runtime_handler(
    State(state): State<HttpAppState>,
    Path(rule_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    match delete_import_learning_rule(runtime.connection(), rule_id, user_id) {
        Ok(true) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": true, "result": true}),
        }),
        Ok(false) => route_response(import_v2_error_response(404, "Rule not found")),
        Err(response) => route_response(response),
    }
}

