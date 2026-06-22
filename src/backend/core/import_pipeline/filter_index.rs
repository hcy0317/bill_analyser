#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CategoryLookup {
    pub name: String,
    pub sub_category: String,
    pub main_category: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AccountLookup {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewFilterIndexItem {
    pub id: i64,
    pub preview_date: String,
    #[serde(rename = "type")]
    pub frontend_type: i64,
    pub source_amount_cents: i64,
    pub category_id: String,
    pub actual_category_name: String,
    pub source_account_id: String,
    pub destination_account_id: String,
    pub actual_source_account_name: String,
    pub actual_destination_account_name: String,
    pub comment: String,
    pub counterparty: String,
    pub payment_method: String,
    pub selected: bool,
    pub is_manually_annotated: bool,
    pub parser_source: String,
    pub parser_tags: Vec<Value>,
    pub dedup_type: String,
    pub dedup_source_ids: Vec<Value>,
    pub transfer_status: Option<String>,
    pub transfer_title: String,
    pub learning_status: Option<String>,
    pub learning_title: String,
    pub learning_summary: String,
    pub learning_mode: String,
    pub recurring_template_id: String,
    pub recurring_candidate_count: i64,
    pub recurring_match_reasons: String,
    pub recurring_matched_date: String,
}

/// 中文说明：把一行导入预览原始 JSON 投影为前端筛选索引，统一分类、账户、parser、去重和信号状态字段。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_import_preview_filter_index_item(
    preview_item: &Map<String, Value>,
    categories_by_id: &BTreeMap<i64, CategoryLookup>,
    accounts_by_id: &BTreeMap<i64, AccountLookup>,
) -> ImportPreviewFilterIndexItem {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "build_import_preview_filter_index_item",
        "business operation entered"
    );
    let category_id_value = preview_item.get("category_id");
    let category_id_key = integer_lookup_key(category_id_value);
    let category_row =
        category_id_key.and_then(|id| categories_by_id.get(&id).map(|row| (id, row)));
    let category_id = category_row
        .map(|(id, _)| id.to_string())
        .unwrap_or_default();
    let actual_category_name = category_row
        .and_then(|(_, row)| first_non_empty([&row.name, &row.sub_category, &row.main_category]))
        .map(ToOwned::to_owned)
        .unwrap_or_default();

    let source_account_id_value = preview_item.get("preview_source_account_id");
    let source_account_id_key = integer_lookup_key(source_account_id_value);
    let source_account_row =
        source_account_id_key.and_then(|id| accounts_by_id.get(&id).map(|row| (id, row)));
    let source_account_id = source_account_row
        .map(|(id, _)| id.to_string())
        .unwrap_or_default();
    let actual_source_account_name = source_account_row
        .map(|(_, row)| row.name.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_default();

    let destination_account_id_value = preview_item.get("preview_destination_account_id");
    let destination_account_id_key = integer_lookup_key(destination_account_id_value);
    let destination_account_row =
        destination_account_id_key.and_then(|id| accounts_by_id.get(&id).map(|row| (id, row)));
    let destination_account_id = destination_account_row
        .map(|(id, _)| id.to_string())
        .unwrap_or_default();
    let actual_destination_account_name = destination_account_row
        .map(|(_, row)| row.name.clone())
        .unwrap_or_default();

    ImportPreviewFilterIndexItem {
        id: integer_field_from_map(preview_item, "id"),
        preview_date: string_field_from_map(preview_item, "preview_date"),
        frontend_type: map_import_preview_type_to_frontend_value(preview_item.get("preview_type")),
        source_amount_cents: integer_field_from_map(preview_item, "preview_amount_cents"),
        category_id,
        actual_category_name,
        source_account_id,
        destination_account_id,
        actual_source_account_name,
        actual_destination_account_name,
        comment: string_field_from_map(preview_item, "preview_description"),
        counterparty: string_field_from_map(preview_item, "preview_counterparty"),
        payment_method: string_field_from_map(preview_item, "preview_payment_method"),
        selected: preview_item
            .get("preview_selected")
            .map(json_value_is_truthy)
            .unwrap_or(true),
        is_manually_annotated: preview_item
            .get("preview_is_manually_annotated")
            .map(json_value_is_truthy)
            .unwrap_or(false),
        parser_source: string_field_from_map(preview_item, "preview_parser_id"),
        parser_tags: list_field_from_map(preview_item, "preview_parser_tags"),
        dedup_type: string_field_from_map(preview_item, "dedup_type"),
        dedup_source_ids: parse_dedup_source_ids(preview_item.get("dedup_source_ids")),
        transfer_status: resolve_import_preview_transfer_signal_status(preview_item),
        transfer_title: matching_section_string_field(preview_item, "transfer", "reason")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| string_field_from_map(preview_item, "transfer_suggestion_reason")),
        learning_status: resolve_import_preview_learning_signal_status(preview_item),
        learning_title: matching_section_string_field(preview_item, "learning", "reason")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                string_field_from_map(preview_item, "learning_recommendation_reason")
            }),
        learning_summary: matching_section_string_field(preview_item, "learning", "summary")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                string_field_from_map(preview_item, "learning_recommendation_summary")
            }),
        learning_mode: matching_section_string_field(preview_item, "learning", "mode")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| string_field_from_map(preview_item, "learning_recommendation_mode")),
        recurring_template_id: normalize_id_text(preview_item.get("preview_recurring_id")),
        recurring_candidate_count: integer_field_from_map(
            preview_item,
            "preview_recurring_candidate_count",
        ),
        recurring_match_reasons: string_field_from_map(
            preview_item,
            "preview_recurring_match_reasons",
        ),
        recurring_matched_date: string_field_from_map(
            preview_item,
            "preview_recurring_matched_date",
        ),
    }
}

/// 中文说明：根据 matching payload 与旧字段判断转账建议是否仍待用户处理，避免已接受或已拒绝的建议重复提示。
#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_import_preview_transfer_signal_status(
    preview_item: &Map<String, Value>,
) -> Option<String> {
    let matching = preview_matching_payload(preview_item);
    let transfer_matching = matching.and_then(|matching| object_field(matching.get("transfer")));
    let review_status = transfer_matching
        .and_then(|matching| matching.get("review_status"))
        .map(value_to_trimmed_lowercase)
        .unwrap_or_default();
    if matches!(review_status.as_str(), "accepted" | "rejected") {
        return Some(review_status);
    }
    let transfer_has_candidate = transfer_matching.is_some_and(|matching| {
        !string_field_from_map(matching, "candidate_type")
            .trim()
            .is_empty()
            || float_field_from_map(matching, "score") > 0.0
            || !string_field_from_map(matching, "reason").trim().is_empty()
    });

    let suggested_preview_type =
        string_field_from_map(preview_item, "suggested_preview_type").to_lowercase();
    let preview_type = string_field_from_map(preview_item, "preview_type").to_lowercase();
    let transfer_score = float_field_from_map(preview_item, "transfer_suggestion_score");
    let transfer_suppressed = transfer_matching
        .and_then(|matching| matching.get("suppressed"))
        .map(json_value_is_truthy)
        .unwrap_or(false);

    if (transfer_has_candidate || matches!(suggested_preview_type.trim(), "转账" | "transfer"))
        && !matches!(preview_type.trim(), "转账" | "transfer")
        && (transfer_score > 0.0 || transfer_has_candidate)
        && !transfer_suppressed
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
    let review_status = learning_matching
        .and_then(|matching| matching.get("review_status"))
        .map(value_to_trimmed_lowercase)
        .unwrap_or_default();
    if matches!(review_status.as_str(), "accepted" | "rejected") {
        return Some(review_status);
    }
    if matches!(review_status.as_str(), "auto_applied" | "auto-applied") {
        return Some("accepted".to_string());
    }

    let learning_rule_id = learning_matching
        .and_then(|matching| matching.get("rule_id"))
        .map(value_to_i64)
        .unwrap_or_default();
    let has_pending_learning =
        (float_field_from_map(preview_item, "learning_recommendation_score") > 0.0
            || !string_field_from_map(preview_item, "learning_recommendation_summary")
                .trim()
                .is_empty()
            || !string_field_from_map(preview_item, "learning_recommendation_reason")
                .trim()
                .is_empty()
            || !string_field_from_map(preview_item, "learning_recommendation_type")
                .trim()
                .is_empty()
            || learning_rule_id > 0)
            && !learning_matching
                .and_then(|matching| matching.get("suppressed"))
                .map(json_value_is_truthy)
                .unwrap_or(false);

    if has_pending_learning {
        Some("pending".to_string())
    } else {
        None
    }
}
