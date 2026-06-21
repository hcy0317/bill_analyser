#[tracing::instrument(level = "debug", skip_all)]
fn import_preview_draft_from_row(row: &ImportPreviewRow) -> ImportPreviewDraft {
    ImportPreviewDraft {
        preview_date: row.preview_date.clone(),
        preview_type: row.preview_type.clone(),
        preview_amount_cents: row.preview_amount_cents,
        preview_destination_amount_cents: row.preview_destination_amount_cents,
        category_id: row.category_id,
        preview_main_category: row.preview_main_category.clone(),
        preview_sub_category: row.preview_sub_category.clone(),
        preview_source_account_id: row.preview_source_account_id,
        preview_destination_account_id: row.preview_destination_account_id,
        preview_counterparty: row.preview_counterparty.clone(),
        preview_payment_method: row.preview_payment_method.clone(),
        preview_description: row.preview_description.clone(),
        preview_parser_id: row.preview_parser_id.clone(),
        preview_parser_tags: Some(json!(row.preview_parser_tags)),
        preview_recurring_id: row.preview_recurring_id,
        preview_recurring_name: row.preview_recurring_name.clone(),
        preview_recurring_candidate_count: row.preview_recurring_candidate_count,
        preview_recurring_match_score: row.preview_recurring_match_score,
        preview_recurring_match_reasons: row.preview_recurring_match_reasons.clone(),
        preview_recurring_matched_date: row.preview_recurring_matched_date.clone(),
        preview_selected: row.preview_selected,
        dedup_type: (!row.dedup_type.trim().is_empty()).then(|| row.dedup_type.clone()),
        dedup_source_ids: row.dedup_source_ids.clone(),
        preview_matching_feedback: row.preview_matching_feedback.clone(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn import_preview_patch_from_draft(preview_id: i64, draft: &ImportPreviewDraft) -> ImportPreviewPatch {
    ImportPreviewPatch::new(preview_id).with_changes([
        (
            ImportPreviewPatchField::Type,
            ImportPreviewPatchValue::Text(draft.preview_type.clone()),
        ),
        (
            ImportPreviewPatchField::MainCategory,
            ImportPreviewPatchValue::Text(draft.preview_main_category.clone()),
        ),
        (
            ImportPreviewPatchField::SubCategory,
            ImportPreviewPatchValue::Text(draft.preview_sub_category.clone()),
        ),
        (
            ImportPreviewPatchField::CategoryId,
            optional_i64_patch_value(draft.category_id),
        ),
        (
            ImportPreviewPatchField::SourceAccountId,
            optional_i64_patch_value(draft.preview_source_account_id),
        ),
        (
            ImportPreviewPatchField::DestinationAccountId,
            optional_i64_patch_value(draft.preview_destination_account_id),
        ),
        (
            ImportPreviewPatchField::DestinationAmount,
            ImportPreviewPatchValue::Integer(draft.preview_destination_amount_cents),
        ),
        (
            ImportPreviewPatchField::RecurringId,
            optional_i64_patch_value(draft.preview_recurring_id),
        ),
        (
            ImportPreviewPatchField::RecurringName,
            ImportPreviewPatchValue::Text(draft.preview_recurring_name.clone()),
        ),
        (
            ImportPreviewPatchField::RecurringCandidateCount,
            ImportPreviewPatchValue::Integer(draft.preview_recurring_candidate_count),
        ),
        (
            ImportPreviewPatchField::RecurringMatchScore,
            ImportPreviewPatchValue::Real(draft.preview_recurring_match_score),
        ),
        (
            ImportPreviewPatchField::RecurringMatchReasons,
            ImportPreviewPatchValue::Text(draft.preview_recurring_match_reasons.clone()),
        ),
        (
            ImportPreviewPatchField::RecurringMatchedDate,
            ImportPreviewPatchValue::Text(draft.preview_recurring_matched_date.clone()),
        ),
        (
            ImportPreviewPatchField::MatchingFeedback,
            ImportPreviewPatchValue::Json(draft.preview_matching_feedback.clone()),
        ),
        (
            ImportPreviewPatchField::Selected,
            ImportPreviewPatchValue::Bool(draft.preview_selected),
        ),
    ])
}

fn optional_i64_patch_value(value: Option<i64>) -> ImportPreviewPatchValue {
    value.map_or(ImportPreviewPatchValue::Null, ImportPreviewPatchValue::Integer)
}
