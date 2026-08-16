/// 解析批量 preview_updates 并落库，confirm/reclassify 依赖它保持前端草稿与 DB 状态一致。
#[tracing::instrument(level = "debug", skip_all)]
fn apply_preview_updates_preserving_selection_from_payload(
    runtime: &mut ImportRuntime,
    session_id: &str,
    user_id: UserId,
    payload: &Value,
) -> Result<usize, ImportV2RouteResponse> {
    let update_items = preview_update_items_from_payload(payload)?;
    let mut patches = Vec::with_capacity(update_items.len());
    for item in update_items {
        let preview_id = preview_id_from_payload(item)?;
        match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
            Ok(Some(preview)) if preview.session_id == session_id => {}
            Ok(_) => return Err(import_v2_error_response(404, "Preview bill not found")),
            Err(error) => return Err(db_error_response(error)),
        }
        patches.push(build_preview_patch_from_payload_with_category_lookup(
            runtime.connection(), user_id, preview_id, item,
        )?);
    }
    apply_preview_patches_preserving_selection(runtime.connection_mut(), session_id, user_id, &patches)
        .map_err(db_error_response)
}

fn preview_ids_from_payload(payload: &Value) -> Vec<i64> {
    let Ok(object) = payload_object(payload) else {
        return Vec::new();
    };
    let mut preview_ids = Vec::new();
    if let Some(value) = first_value(object, &["preview_ids", "previewIds"]) {
        match value {
            Value::Array(values) => {
                preview_ids.extend(values.iter().filter_map(value_to_i64).filter(|id| *id > 0));
            }
            other => {
                if let Some(preview_id) = value_to_i64(other).filter(|id| *id > 0) {
                    preview_ids.push(preview_id);
                }
            }
        }
    }
    if preview_ids.is_empty() {
        if let Ok(items) = preview_update_items_from_payload(payload) {
            preview_ids.extend(
                items
                    .iter()
                    .filter_map(|item| preview_id_from_payload(item).ok()),
            );
        }
    }
    preview_ids.sort_unstable();
    preview_ids.dedup();
    preview_ids
}

fn expected_state_from_payload(
    object: &Map<String, Value>,
) -> Result<ImportPreviewExpectedState, ImportV2RouteResponse> {
    let expected_state = first_value(object, &["expectedState", "expected_state"])
        .and_then(Value::as_object)
        .ok_or_else(|| import_v2_error_response(400, "Invalid request"))?;
    Ok(ImportPreviewExpectedState {
        session_id: first_value(expected_state, &["sessionId", "session_id"])
            .and_then(value_to_text),
        expected_row_version: None,
        review_status: first_value(
            expected_state,
            &["reviewStatus", "review_status", "status"],
        )
        .and_then(value_to_text)
        .map(|value| value.trim().to_ascii_lowercase()),
        preview_type: first_value(expected_state, &["type", "previewType", "preview_type"])
            .and_then(value_to_text),
        preview_category_id: optional_id_field_from_object(
            expected_state,
            &[
                "categoryId",
                "category_id",
                "previewCategoryId",
                "preview_category_id",
            ],
        ),
        preview_main_category: first_value(
            expected_state,
            &[
                "mainCategory",
                "previewMainCategory",
                "preview_main_category",
            ],
        )
        .and_then(value_to_text),
        preview_sub_category: first_value(
            expected_state,
            &["subCategory", "previewSubCategory", "preview_sub_category"],
        )
        .and_then(value_to_text),
        preview_recurring_id: optional_id_field_from_object(
            expected_state,
            &[
                "recurringTemplateId",
                "recurringId",
                "previewRecurringId",
                "preview_recurring_id",
            ],
        ),
        preview_source_account_id: optional_id_field_from_object(
            expected_state,
            &[
                "sourceAccountId",
                "previewSourceAccountId",
                "preview_source_account_id",
            ],
        ),
        preview_destination_account_id: optional_id_field_from_object(
            expected_state,
            &[
                "destinationAccountId",
                "previewDestinationAccountId",
                "preview_destination_account_id",
            ],
        ),
        preview_matching_feedback: first_value(
            expected_state,
            &[
                "matchingFeedback",
                "previewMatchingFeedback",
                "preview_matching_feedback",
            ],
        )
        .cloned(),
    })
}

fn optional_id_field_from_object(
    object: &Map<String, Value>,
    keys: &[&str],
) -> Option<Option<i64>> {
    first_value(object, keys).map(|value| match value_to_i64(value) {
        Some(value) if value > 0 => Some(value),
        _ => None,
    })
}
