/// 中文说明：按优先级从 review_status/status/lifecycle_status/signal_state 取第一个非空规范化值。
pub fn resolve_first_nonempty_status(section: &Map<String, Value>) -> String {
    for field in IMPORT_PREVIEW_SIGNAL_STATUS_FIELDS {
        let value = section
            .get(*field)
            .map(import_preview_signal_value_to_lowercase)
            .unwrap_or_default();
        if !value.is_empty() {
            return value;
        }
    }
    String::new()
}

/// 中文说明：根据 LLM matching 状态判断建议是否在可见信号列中展示。
#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_import_preview_llm_signal_status(
    preview_item: &Map<String, Value>,
) -> Option<String> {
    let matching = preview_matching_payload(preview_item);
    let llm_matching = matching.and_then(|matching| object_field(matching.get("llm")))?;
    // Suppressed/none check first
    if matching_section_is_suppressed_or_none(llm_matching) {
        return None;
    }
    if matching_section_has_unknown_review_status(
        llm_matching,
        IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES,
    ) {
        return None;
    }
    if !llm_matching_section_is_meaningful(llm_matching) {
        return None;
    }
    let status = resolve_first_nonempty_status(llm_matching);
    if matches!(status.as_str(), "accepted" | "rejected" | "skipped") {
        return Some(status);
    }
    if matches!(status.as_str(), "auto_applied" | "auto-applied") {
        return Some("accepted".to_string());
    }
    Some("pending".to_string())
}

/// 中文说明：根据 matching payload 与旧字段判断转账建议是否仍待用户处理，避免已接受或已拒绝的建议重复提示。
#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_import_preview_transfer_signal_status(
    preview_item: &Map<String, Value>,
) -> Option<String> {
    let matching = preview_matching_payload(preview_item);
    let transfer_matching = matching.and_then(|matching| object_field(matching.get("transfer")));
    if transfer_matching.is_some_and(matching_section_is_suppressed_or_none) {
        return None;
    }
    let resolved_status = transfer_matching
        .map(resolve_first_nonempty_status)
        .unwrap_or_default();
    if !resolved_status.is_empty()
        && !IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES.contains(&resolved_status.as_str())
    {
        return None;
    }
    if matches!(
        resolved_status.as_str(),
        "accepted" | "rejected" | "skipped"
    ) {
        return Some(resolved_status);
    }
    if matches!(resolved_status.as_str(), "auto_applied" | "auto-applied") {
        return Some("accepted".to_string());
    }
    if resolved_status == "pending" {
        return Some("pending".to_string());
    }
    let transfer_has_candidate = transfer_matching.is_some_and(|matching| {
        matching
            .get("candidate_type")
            .and_then(Value::as_str)
            .is_some_and(|value| !trim_import_preview_signal_text(value).is_empty())
            || json_number_is_positive(matching.get("score").unwrap_or(&Value::Null))
            || matching
                .get("reason")
                .and_then(Value::as_str)
                .is_some_and(|value| !trim_import_preview_signal_text(value).is_empty())
    });

    let suggested_preview_type =
        string_field_from_map(preview_item, "suggested_preview_type").to_ascii_lowercase();
    let preview_type = string_field_from_map(preview_item, "preview_type").to_ascii_lowercase();
    let transfer_score = json_number_is_positive(
        preview_item
            .get("transfer_suggestion_score")
            .unwrap_or(&Value::Null),
    );
    if (transfer_has_candidate || matches!(suggested_preview_type.trim(), "转账" | "transfer"))
        && !matches!(preview_type.trim(), "转账" | "transfer" | "4")
        && (transfer_score || transfer_has_candidate)
    {
        Some("pending".to_string())
    } else {
        None
    }
}

/// 中文说明：根据 learning matching 状态和旧推荐字段判断学习建议展示状态，兼容 auto-applied 与 suppressed 语义。
#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_import_preview_learning_signal_status(
    preview_item: &Map<String, Value>,
) -> Option<String> {
    let matching = preview_matching_payload(preview_item);
    let learning_matching = matching.and_then(|matching| object_field(matching.get("learning")));
    // Suppressed check first: explicit suppressed flag OR resolved none/suppressed hides everything
    if learning_matching.is_some_and(matching_section_is_suppressed_or_none) {
        return None;
    }
    let review_status = learning_matching
        .map(resolve_first_nonempty_status)
        .unwrap_or_default();
    if !review_status.is_empty()
        && !IMPORT_PREVIEW_LEARNING_CANONICAL_STATUSES.contains(&review_status.as_str())
    {
        return None;
    }
    if matches!(review_status.as_str(), "accepted" | "rejected" | "skipped") {
        return Some(review_status);
    }
    if matches!(review_status.as_str(), "auto_applied" | "auto-applied") {
        return Some("accepted".to_string());
    }
    if !learning_matching.is_some_and(learning_matching_section_is_meaningful)
        && !legacy_learning_fields_are_meaningful(preview_item)
    {
        return None;
    }
    if !review_status.is_empty() && !matches!(review_status.as_str(), "pending") {
        return Some("pending".to_string());
    }

    let learning_rule_id = learning_matching
        .and_then(|matching| matching.get("rule_id"))
        .map(value_to_i64)
        .unwrap_or_default();
    let has_pending_learning = (learning_matching
        .is_some_and(learning_matching_section_is_meaningful)
        || json_number_is_positive(
            preview_item
                .get("learning_recommendation_score")
                .unwrap_or(&Value::Null),
        )
        || learning_matching.is_some_and(|matching| {
            json_number_is_positive(matching.get("score").unwrap_or(&Value::Null))
        })
        || !trim_import_preview_signal_text(&string_field_from_map(
            preview_item,
            "learning_recommendation_summary",
        ))
        .is_empty()
        || !trim_import_preview_signal_text(&string_field_from_map(
            preview_item,
            "learning_recommendation_reason",
        ))
        .is_empty()
        || !trim_import_preview_signal_text(&string_field_from_map(
            preview_item,
            "learning_recommendation_type",
        ))
        .is_empty()
        || learning_rule_id > 0)
        && !learning_matching
            .and_then(|matching| matching.get("suppressed"))
            .map(import_preview_signal_value_is_truthy)
            .unwrap_or(false);

    if has_pending_learning {
        Some("pending".to_string())
    } else {
        None
    }
}

fn legacy_learning_fields_are_meaningful(preview_item: &Map<String, Value>) -> bool {
    json_number_is_positive(
        preview_item
            .get("learning_recommendation_score")
            .unwrap_or(&Value::Null),
    ) || json_text_is_meaningful(
        preview_item
            .get("learning_recommendation_summary")
            .unwrap_or(&Value::Null),
    ) || json_text_is_meaningful(
        preview_item
            .get("learning_recommendation_reason")
            .unwrap_or(&Value::Null),
    ) || json_text_is_meaningful(
        preview_item
            .get("learning_recommendation_type")
            .unwrap_or(&Value::Null),
    )
}

fn build_import_preview_llm_category_path(preview_item: &Map<String, Value>) -> String {
    [
        matching_section_string_field(preview_item, "llm", "suggested_main_category")
            .unwrap_or_default(),
        matching_section_string_field(preview_item, "llm", "suggested_sub_category")
            .unwrap_or_default(),
    ]
    .into_iter()
    .filter(|part| !part.trim().is_empty())
    .collect::<Vec<_>>()
    .join("/")
}
