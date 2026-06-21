/// 处理导入 confirm 请求，解析 preview_updates 与历史 ack 后交给 DB 事务完成正式账单写入。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_confirm_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "import_confirm_runtime_handler", "business operation entered");
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
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
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
        let preserve_unpatched_selection = first_value(
            object,
            &["preserve_unpatched_selection", "preserveUnpatchedSelection"],
        )
        .and_then(Value::as_bool)
        .unwrap_or(false);
        let patch_result = if preserve_unpatched_selection {
            apply_preview_patches_preserving_selection(
                runtime.connection_mut(),
                &session_id,
                user_id,
                &patches,
            )
        } else {
            replace_preview_selection_with_patches(
                runtime.connection_mut(),
                &session_id,
                user_id,
                &patches,
            )
        };
        if let Err(error) = patch_result {
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

    let history_acknowledgement = match history_rewrite_acknowledgement_from_payload(object) {
        Ok(acknowledgement) => acknowledgement,
        Err(response) => return route_response(response),
    };
    let result = match confirm_preview_to_bills_with_ack(
        runtime.connection_mut(),
        &session_id,
        user_id,
        history_acknowledgement.as_ref(),
    ) {
        Ok(result) => result,
        Err(bill_analyser_db::DbError::InvalidOperation(message)) => {
            return route_response(import_v2_error_response(400, &message));
        }
        Err(error) => return route_response(db_error_response(error)),
    };
    route_response(import_stage_confirm_success(ImportStageConfirmData {
        imported_count: result.confirmed_count,
        skipped_count: result.skipped_count + result.duplicate_count,
        errors: result.errors,
    }))
}

fn history_rewrite_acknowledgement_from_payload(
    object: &Map<String, Value>,
) -> Result<Option<ImportHistoryRewriteAcknowledgement>, ImportV2RouteResponse> {
    let Some(value) = first_value(
        object,
        &[
            "history_rewrite_acknowledgement",
            "historyRewriteAcknowledgement",
            "history_rewrite_ack",
            "historyRewriteAck",
        ],
    ) else {
        return Ok(None);
    };
    serde_json::from_value::<ImportHistoryRewriteAcknowledgement>(value.clone())
        .map(Some)
        .map_err(|_| import_v2_error_response(400, "Invalid history rewrite acknowledgement"))
}
