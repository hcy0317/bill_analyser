fn normalize_transfer_snapshot(value: &Value) -> Option<Value> {
    value
        .as_object()
        .map(|object| Value::Object(object.clone()))
        .filter(|value| value.as_object().is_some_and(|object| !object.is_empty()))
}

fn build_transfer_previous_snapshot(preview: &ImportPreviewRow) -> Value {
    serde_json::json!({
        "preview_type": preview.preview_type,
        "preview_main_category": preview.preview_main_category,
        "preview_sub_category": preview.preview_sub_category,
        "preview_recurring_id": preview.preview_recurring_id,
        "preview_recurring_name": preview.preview_recurring_name,
        "preview_recurring_candidate_count": preview.preview_recurring_candidate_count,
        "preview_recurring_match_score": preview.preview_recurring_match_score,
        "preview_recurring_match_reasons": preview.preview_recurring_match_reasons,
        "preview_recurring_matched_date": preview.preview_recurring_matched_date,
    })
}

fn transfer_snapshot_restore_changes(
    snapshot: &Value,
) -> Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)> {
    vec![
        (
            ImportPreviewPatchField::Type,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_type")),
        ),
        (
            ImportPreviewPatchField::MainCategory,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_main_category")),
        ),
        (
            ImportPreviewPatchField::SubCategory,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_sub_category")),
        ),
        (
            ImportPreviewPatchField::RecurringId,
            snapshot_optional_i64(snapshot, "preview_recurring_id")
                .map(ImportPreviewPatchValue::Integer)
                .unwrap_or(ImportPreviewPatchValue::Null),
        ),
        (
            ImportPreviewPatchField::RecurringName,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_recurring_name")),
        ),
        (
            ImportPreviewPatchField::RecurringCandidateCount,
            ImportPreviewPatchValue::Integer(snapshot_i64(
                snapshot,
                "preview_recurring_candidate_count",
            )),
        ),
        (
            ImportPreviewPatchField::RecurringMatchScore,
            ImportPreviewPatchValue::Real(snapshot_f64(snapshot, "preview_recurring_match_score")),
        ),
        (
            ImportPreviewPatchField::RecurringMatchReasons,
            ImportPreviewPatchValue::Text(snapshot_text(
                snapshot,
                "preview_recurring_match_reasons",
            )),
        ),
        (
            ImportPreviewPatchField::RecurringMatchedDate,
            ImportPreviewPatchValue::Text(snapshot_text(
                snapshot,
                "preview_recurring_matched_date",
            )),
        ),
    ]
}

fn normalize_learning_snapshot(value: &Value) -> Option<Value> {
    value.as_object().map(|_| {
        serde_json::json!({
            "preview_type": snapshot_text(value, "preview_type"),
            "preview_main_category": snapshot_text(value, "preview_main_category"),
            "preview_sub_category": snapshot_text(value, "preview_sub_category"),
            "preview_source_account_id": snapshot_account_id(value, "preview_source_account_id"),
            "preview_destination_account_id": snapshot_account_id(value, "preview_destination_account_id"),
        })
    })
}

fn build_learning_previous_snapshot(preview: &ImportPreviewRow) -> Value {
    serde_json::json!({
        "preview_type": preview.preview_type,
        "preview_main_category": preview.preview_main_category,
        "preview_sub_category": preview.preview_sub_category,
        "preview_source_account_id": preview.preview_source_account_id,
        "preview_destination_account_id": preview.preview_destination_account_id,
    })
}

fn learning_accept_preview_snapshot(
    applied_result: Option<&ImportPreviewLearningApply>,
    preview: &ImportPreviewRow,
) -> Value {
    serde_json::json!({
        "preview_type": applied_result
            .and_then(|value| value.preview_type.as_ref())
            .cloned()
            .unwrap_or_else(|| preview.preview_type.clone()),
        "preview_main_category": applied_result
            .and_then(|value| value.preview_main_category.as_ref())
            .cloned()
            .unwrap_or_else(|| preview.preview_main_category.clone()),
        "preview_sub_category": applied_result
            .and_then(|value| value.preview_sub_category.as_ref())
            .cloned()
            .unwrap_or_else(|| preview.preview_sub_category.clone()),
        "preview_source_account_id": applied_result
            .and_then(|value| value.preview_source_account_id)
            .unwrap_or(preview.preview_source_account_id),
        "preview_destination_account_id": applied_result
            .and_then(|value| value.preview_destination_account_id)
            .unwrap_or(preview.preview_destination_account_id),
    })
}

fn learning_snapshot_restore_changes(
    snapshot: &Value,
) -> Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)> {
    vec![
        (
            ImportPreviewPatchField::Type,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_type")),
        ),
        (
            ImportPreviewPatchField::MainCategory,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_main_category")),
        ),
        (
            ImportPreviewPatchField::SubCategory,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_sub_category")),
        ),
        (
            ImportPreviewPatchField::SourceAccountId,
            snapshot_account_id(snapshot, "preview_source_account_id")
                .map(ImportPreviewPatchValue::Integer)
                .unwrap_or(ImportPreviewPatchValue::Null),
        ),
        (
            ImportPreviewPatchField::DestinationAccountId,
            snapshot_account_id(snapshot, "preview_destination_account_id")
                .map(ImportPreviewPatchValue::Integer)
                .unwrap_or(ImportPreviewPatchValue::Null),
        ),
    ]
}

fn learning_preview_matches_snapshot(preview: &ImportPreviewRow, snapshot: &Value) -> bool {
    snapshot_text(snapshot, "preview_type") == preview.preview_type
        && snapshot_text(snapshot, "preview_main_category") == preview.preview_main_category
        && snapshot_text(snapshot, "preview_sub_category") == preview.preview_sub_category
        && snapshot_account_id(snapshot, "preview_source_account_id")
            == preview.preview_source_account_id
        && snapshot_account_id(snapshot, "preview_destination_account_id")
            == preview.preview_destination_account_id
}

fn preview_llm_not_found() -> ImportPreviewLlmDecisionResult {
    ImportPreviewLlmDecisionResult {
        preview: None,
        event_id: None,
        applied_fields: Vec::new(),
        restored: false,
    }
}

fn build_llm_previous_snapshot(preview: &ImportPreviewRow) -> Value {
    serde_json::json!({
        "preview_main_category": preview.preview_main_category,
        "preview_sub_category": preview.preview_sub_category,
        "preview_source_account_id": preview.preview_source_account_id,
        "preview_destination_account_id": preview.preview_destination_account_id,
    })
}

fn normalize_llm_snapshot(value: &Value) -> Option<Value> {
    value.as_object().map(|_| {
        serde_json::json!({
            "preview_main_category": snapshot_text(value, "preview_main_category"),
            "preview_sub_category": snapshot_text(value, "preview_sub_category"),
            "preview_source_account_id": snapshot_account_id(value, "preview_source_account_id"),
            "preview_destination_account_id": snapshot_account_id(value, "preview_destination_account_id"),
        })
    })
}

fn llm_snapshot_restore_changes(
    snapshot: &Value,
) -> Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)> {
    vec![
        (
            ImportPreviewPatchField::MainCategory,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_main_category")),
        ),
        (
            ImportPreviewPatchField::SubCategory,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_sub_category")),
        ),
        (
            ImportPreviewPatchField::SourceAccountId,
            snapshot_account_id(snapshot, "preview_source_account_id")
                .map(ImportPreviewPatchValue::Integer)
                .unwrap_or(ImportPreviewPatchValue::Null),
        ),
        (
            ImportPreviewPatchField::DestinationAccountId,
            snapshot_account_id(snapshot, "preview_destination_account_id")
                .map(ImportPreviewPatchValue::Integer)
                .unwrap_or(ImportPreviewPatchValue::Null),
        ),
    ]
}

fn llm_preview_matches_snapshot(preview: &ImportPreviewRow, snapshot: &Value) -> bool {
    snapshot_text(snapshot, "preview_main_category") == preview.preview_main_category
        && snapshot_text(snapshot, "preview_sub_category") == preview.preview_sub_category
        && snapshot_account_id(snapshot, "preview_source_account_id")
            == preview.preview_source_account_id
        && snapshot_account_id(snapshot, "preview_destination_account_id")
            == preview.preview_destination_account_id
}
