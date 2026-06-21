fn llm_suggestion_from_value(value: &Value) -> Option<ImportPreviewLlmSuggestion> {
    let object = value.as_object()?;
    let suggestion = ImportPreviewLlmSuggestion {
        suggested_type: first_value(
            object,
            &[
                "suggested_type",
                "suggestedType",
                "previewType",
                "preview_type",
                "type",
            ],
        )
        .and_then(value_to_preview_type_text)
        .unwrap_or_default(),
        suggested_category_id: first_value(
            object,
            &[
                "suggested_category_id",
                "suggestedCategoryId",
                "categoryId",
                "category_id",
            ],
        )
        .and_then(value_to_i64)
        .filter(|value| *value > 0),
        suggested_main_category: first_value(
            object,
            &[
                "suggested_main_category",
                "suggestedMainCategory",
                "mainCategory",
                "main_category",
            ],
        )
        .and_then(value_to_text)
        .unwrap_or_default(),
        suggested_sub_category: first_value(
            object,
            &[
                "suggested_sub_category",
                "suggestedSubCategory",
                "subCategory",
                "sub_category",
            ],
        )
        .and_then(value_to_text)
        .unwrap_or_default(),
        suggested_source_account: first_value(
            object,
            &[
                "suggested_source_account",
                "suggestedSourceAccount",
                "sourceAccount",
                "source_account",
            ],
        )
        .and_then(value_to_text)
        .unwrap_or_default(),
        suggested_destination_account: first_value(
            object,
            &[
                "suggested_destination_account",
                "suggestedDestinationAccount",
                "destinationAccount",
                "destination_account",
            ],
        )
        .and_then(value_to_text)
        .unwrap_or_default(),
        resolved_source_account_id: first_value(
            object,
            &[
                "resolved_source_account_id",
                "resolvedSourceAccountId",
                "sourceAccountId",
                "source_account_id",
            ],
        )
        .and_then(value_to_i64)
        .filter(|value| *value > 0),
        resolved_destination_account_id: first_value(
            object,
            &[
                "resolved_destination_account_id",
                "resolvedDestinationAccountId",
                "destinationAccountId",
                "destination_account_id",
            ],
        )
        .and_then(value_to_i64)
        .filter(|value| *value > 0),
        confidence: first_value(object, &["confidence"])
            .and_then(value_to_f64)
            .unwrap_or_default(),
        reason: first_value(object, &["reason"])
            .and_then(value_to_text)
            .unwrap_or_default(),
    };
    Some(suggestion)
}

/// 把 LLM 建议审核结果投影为 HTTP response，保留建议字段与当前 preview item 同步状态。
fn llm_decision_result_response(
    result: ImportPreviewLlmDecisionResult,
    session_id: &str,
    preview_id: i64,
    decision: &str,
) -> ImportV2RouteResponse {
    let Some(preview) = result.preview else {
        return import_v2_error_response(404, "Preview recommendation is no longer available");
    };
    let llm_payload = preview
        .preview_matching_feedback
        .get("llm")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let preview = preview_row_to_value(preview);
    import_v2_data_response(json!({
        "session_id": session_id,
        "preview_id": preview_id,
        "preview": preview,
        "matching": {"llm": llm_payload},
        "event_id": result.event_id,
        "applied_fields": result.applied_fields,
        "decision": decision,
    }))
}
