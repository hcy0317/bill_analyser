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
    let mut preview = match get_preview_by_session(runtime.connection(), &session_id, user_id, false) {
        Ok(preview) => preview,
        Err(error) => return route_response(db_error_response(error)),
    };
    let mut intelligent_drafts = preview
        .iter()
        .map(import_preview_draft_from_row)
        .collect::<Vec<_>>();
    if let Err(error) = apply_import_intelligence_chain(
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
    preview = match get_preview_by_session(runtime.connection(), &session_id, user_id, false) {
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
