fn push_ilike_predicate(query: &mut QueryBuilder<'_, Postgres>, column: &str, value: &str) {
    query.push(" AND ");
    query.push(column);
    query.push(" ILIKE ");
    query.push_bind(like_pattern(value));
}

fn push_category_predicate(query: &mut QueryBuilder<'_, Postgres>, alias: &str, value: &str) {
    if value == "__none__" {
        query.push(" AND ");
        query.push(alias);
        query.push(".category_id IS NULL AND COALESCE(");
        query.push(alias);
        query.push(".preview_payload->>'preview_main_category', '') = '' AND COALESCE(");
        query.push(alias);
        query.push(".preview_payload->>'preview_sub_category', '') = ''");
        return;
    }
    if value == "__invalid__" {
        query.push(" AND ");
        push_preview_missing_category_condition(query, alias);
        return;
    }
    let Some(category_id) = parse_positive_identity_filter_id(value) else {
        query.push(" AND FALSE");
        return;
    };
    query.push(" AND ");
    query.push(alias);
    query.push(".category_id = ");
    query.push_bind(category_id);
}

fn push_account_predicate(query: &mut QueryBuilder<'_, Postgres>, alias: &str, value: &str) {
    if value == "__none__" {
        query.push(" AND ");
        query.push(alias);
        query.push(".account_id IS NULL AND ");
        query.push(alias);
        query.push(".transfer_target_account_id IS NULL AND COALESCE(");
        query.push(alias);
        query.push(".payment_method, '') = ''");
        return;
    }
    if value == "__invalid__" {
        query.push(" AND ");
        push_preview_account_filter_invalid_condition(query, alias);
        return;
    }
    let Some(account_id) = parse_positive_identity_filter_id(value) else {
        query.push(" AND FALSE");
        return;
    };
    query.push(" AND (");
    query.push(alias);
    query.push(".account_id = ");
    query.push_bind(account_id);
    query.push(" OR ");
    query.push(alias);
    query.push(".transfer_target_account_id = ");
    query.push_bind(account_id);
    query.push(")");
}

fn parse_positive_identity_filter_id(value: &str) -> Option<i64> {
    value.trim().parse::<i64>().ok().filter(|id| *id > 0)
}

fn push_tag_predicate(query: &mut QueryBuilder<'_, Postgres>, alias: &str, value: &str) {
    if matches!(value, "__none__" | "__invalid__") {
        query.push(" AND ");
        push_preview_empty_parser_tags_condition(query, alias);
        return;
    }
    query.push(" AND ");
    query.push(alias);
    query.push(".preview_payload->>'preview_parser_tags' ILIKE ");
    query.push_bind(like_pattern(value));
}

fn push_preview_signal_predicate(query: &mut QueryBuilder<'_, Postgres>, alias: &str, value: &str) {
    let filter = normalize_visible_signal_filter(value);
    let Some((family, status)) = signal_filter_family_status(&filter) else {
        query.push(" AND ");
        push_preview_signal_family_condition(query, alias, &filter);
        return;
    };

    query.push(" AND (");
    push_preview_signal_family_condition(query, alias, family);
    query.push(" AND ");
    push_preview_feedback_family_text_search(query, alias, family, status);
    query.push(")");
}

fn push_preview_signal_family_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
    family: &str,
) {
    match family {
        family
            if ImportPreviewSignalFamily::parse(family)
                == Some(ImportPreviewSignalFamily::Parser) =>
        {
            query.push("(");
            push_preview_parser_signal_condition(query, alias);
            query.push(" AND NOT (");
            push_preview_specific_visible_signal_condition(query, alias);
            query.push("))");
        }
        family
            if ImportPreviewSignalFamily::parse(family)
                == Some(ImportPreviewSignalFamily::PlatformDuplicate) =>
        {
            push_preview_platform_duplicate_signal_condition(query, alias)
        }
        family
            if ImportPreviewSignalFamily::parse(family)
                == Some(ImportPreviewSignalFamily::Transfer) =>
        {
            push_preview_transfer_signal_condition(query, alias)
        }
        family
            if ImportPreviewSignalFamily::parse(family)
                == Some(ImportPreviewSignalFamily::History) =>
        {
            push_preview_history_signal_condition(query, alias)
        }
        family
            if ImportPreviewSignalFamily::parse(family)
                == Some(ImportPreviewSignalFamily::Learning) =>
        {
            push_preview_recommendation_signal_condition(query, alias)
        }
        family
            if ImportPreviewSignalFamily::parse(family) == Some(ImportPreviewSignalFamily::Llm) =>
        {
            push_preview_llm_signal_condition(query, alias)
        }
        _ => {
            query.push("FALSE");
        }
    }
}

fn push_preview_parser_signal_condition(query: &mut QueryBuilder<'_, Postgres>, alias: &str) {
    query.push("(COALESCE(NULLIF(btrim(");
    query.push(alias);
    query.push(".preview_payload->>'preview_parser_id', ");
    query.push_bind(IMPORT_PREVIEW_SIGNAL_TRIM_CHARS);
    query.push("), ''), '') <> '' OR EXISTS (SELECT 1 FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(");
    query.push(alias);
    query.push(".preview_payload->'preview_parser_tags') = 'array' THEN ");
    query.push(alias);
    query.push(".preview_payload->'preview_parser_tags' ELSE '[]'::jsonb END) AS parser_tag(value) WHERE btrim(parser_tag.value, ");
    query.push_bind(IMPORT_PREVIEW_SIGNAL_TRIM_CHARS);
    query.push(") <> '') OR ");
    push_preview_feedback_key_condition(query, alias, "parser");
    query.push(")");
}

fn push_preview_specific_visible_signal_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    push_preview_platform_duplicate_signal_condition(query, alias);
    query.push(" OR ");
    push_preview_transfer_signal_condition(query, alias);
    query.push(" OR ");
    push_preview_history_signal_condition(query, alias);
    query.push(" OR ");
    push_preview_recommendation_signal_condition(query, alias);
    query.push(" OR ");
    push_preview_llm_signal_condition(query, alias);
}

fn push_preview_platform_duplicate_signal_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    push_preview_dedup_type_in_condition(query, alias, &["platform_bank"]);
}

fn push_preview_transfer_signal_condition(query: &mut QueryBuilder<'_, Postgres>, alias: &str) {
    push_preview_feedback_key_condition(query, alias, "transfer");
    query.push(" AND NOT ");
    push_preview_feedback_truthy_field_condition(query, alias, "transfer", "suppressed");
    query.push(" AND ");
    push_preview_feedback_resolved_status_expr(query, alias, "transfer");
    query.push(" NOT IN (");
    push_sql_string_list(query, IMPORT_PREVIEW_SIGNAL_SUPPRESSED_STATUSES);
    query.push(") AND (");
    push_preview_feedback_resolved_status_expr(query, alias, "transfer");
    query.push(" = 'pending' OR (");
    push_preview_feedback_resolved_status_expr(query, alias, "transfer");
    query.push(" = '' AND LOWER(btrim(COALESCE(");
    query.push(alias);
    query.push(".preview_payload->>'preview_type', ''), ");
    query.push_bind(IMPORT_PREVIEW_SIGNAL_TRIM_CHARS);
    query.push(")) NOT IN ('转账', 'transfer', '4') AND (");
    push_preview_feedback_string_type_check(query, alias, "transfer", "candidate_type");
    query.push(" AND ");
    push_preview_feedback_text_expr(query, alias, "transfer", "candidate_type");
    query.push(" <> '' OR ");
    push_preview_feedback_string_type_check(query, alias, "transfer", "reason");
    query.push(" AND ");
    push_preview_feedback_text_expr(query, alias, "transfer", "reason");
    query.push(" <> '' OR ");
    push_preview_feedback_positive_number_condition(query, alias, "transfer", "score");
    query.push(")))");
}

fn push_preview_recommendation_signal_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    push_preview_meaningful_feedback_condition(
        query,
        alias,
        "learning",
        IMPORT_PREVIEW_LEARNING_NUMERIC_EVIDENCE_FIELDS,
        IMPORT_PREVIEW_LEARNING_TEXT_EVIDENCE_FIELDS,
    );
}

fn push_preview_llm_signal_condition(query: &mut QueryBuilder<'_, Postgres>, alias: &str) {
    push_preview_meaningful_feedback_condition(
        query,
        alias,
        "llm",
        IMPORT_PREVIEW_LLM_NUMERIC_EVIDENCE_FIELDS,
        IMPORT_PREVIEW_LLM_TEXT_EVIDENCE_FIELDS,
    );
}

fn push_preview_history_signal_condition(query: &mut QueryBuilder<'_, Postgres>, alias: &str) {
    query.push("(");
    push_preview_feedback_text_expr(query, alias, "reconciliation", "planned_operation");
    query.push(" IN (");
    push_sql_string_list(query, IMPORT_PREVIEW_HISTORY_OPERATION_NAMES);
    query.push(") OR ");
    push_preview_feedback_truthy_field_condition(
        query,
        alias,
        "reconciliation",
        "destructive_ack_required",
    );
    query.push(")");
}

fn push_preview_feedback_resolved_status_expr(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
    key: &str,
) {
    let mut first = true;
    query.push("COALESCE(");
    for status_field in IMPORT_PREVIEW_SIGNAL_STATUS_FIELDS {
        if !first {
            query.push(", ");
        }
        query.push("NULLIF(LOWER(");
        push_preview_feedback_text_expr(query, alias, key, status_field);
        query.push("), '')");
        first = false;
    }
    query.push(", '')");
}

fn push_preview_meaningful_feedback_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
    key: &str,
    numeric_fields: &[&str],
    text_fields: &[&str],
) {
    let canonical_statuses = if key == "learning" {
        IMPORT_PREVIEW_LEARNING_CANONICAL_STATUSES
    } else {
        IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES
    };
    query.push("(");
    push_preview_feedback_key_condition(query, alias, key);
    query.push(" AND NOT ");
    push_preview_feedback_truthy_field_condition(query, alias, key, "suppressed");
    query.push(" AND ");
    push_preview_feedback_resolved_status_expr(query, alias, key);
    query.push(" NOT IN (");
    push_sql_string_list(query, IMPORT_PREVIEW_SIGNAL_SUPPRESSED_STATUSES);
    query.push(") AND (");
    push_preview_feedback_resolved_status_expr(query, alias, key);
    query.push(" = '' OR ");
    push_preview_feedback_resolved_status_expr(query, alias, key);
    query.push(" IN (");
    push_sql_string_list(query, canonical_statuses);
    query.push(")) AND (");
    push_preview_feedback_resolved_status_expr(query, alias, key);
    query.push(" IN (");
    push_sql_string_list(query, IMPORT_PREVIEW_SIGNAL_TERMINAL_STATUSES);
    query.push(") OR (");
    push_preview_feedback_resolved_status_expr(query, alias, key);
    query.push(" <> '' AND ");
    push_preview_feedback_resolved_status_expr(query, alias, key);
    query.push(" NOT IN (");
    push_sql_string_list(query, IMPORT_PREVIEW_SIGNAL_NON_PENDING_EXCLUDED_STATUSES);
    query.push("))");
    for field in numeric_fields {
        query.push(" OR ");
        push_preview_feedback_positive_number_condition(query, alias, key, field);
    }
    for field in text_fields {
        query.push(" OR ");
        push_preview_feedback_meaningful_text_condition(query, alias, key, field);
    }
    query.push("))");
}

fn push_preview_feedback_text_expr(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
    key: &str,
    field: &str,
) {
    query.push("btrim(COALESCE(");
    query.push(alias);
    query.push(".preview_payload#>>'{preview_matching_feedback,");
    query.push(key);
    query.push(",");
    query.push(field);
    query.push("}', ''), ");
    query.push_bind(IMPORT_PREVIEW_SIGNAL_TRIM_CHARS);
    query.push(")");
}

fn push_preview_feedback_text_in_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
    key: &str,
    field: &str,
    values: &[&str],
) {
    query.push("LOWER(");
    push_preview_feedback_text_expr(query, alias, key, field);
    query.push(") IN (");
    push_sql_string_list(query, values);
    query.push(")");
}

/// 中文说明：信号真值 SQL 谓词。与 Rust 侧 `import_preview_signal_value_is_truthy` 保持一致：
/// 文本 true/yes/y 直接命中，或十进制数字文本解析为有限非零 double precision 亦为真。
fn push_preview_feedback_truthy_field_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
    key: &str,
    field: &str,
) {
    query.push("(");
    push_preview_feedback_text_in_condition(
        query,
        alias,
        key,
        field,
        IMPORT_PREVIEW_SIGNAL_TRUTHY_TEXT_VALUES,
    );
    query.push(" OR (");
    push_preview_feedback_text_expr(query, alias, key, field);
    query.push(" ~ ");
    query.push_bind(IMPORT_PREVIEW_DECIMAL_NUMERIC_STRING_GRAMMAR);
    query.push(" AND ");
    push_preview_feedback_text_expr(query, alias, key, field);
    query.push(" ~ ");
    query.push_bind(IMPORT_PREVIEW_DECIMAL_HAS_NON_ZERO);
    query.push("))");
}

fn push_preview_feedback_positive_number_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
    key: &str,
    field: &str,
) {
    query.push("(");
    push_preview_feedback_text_expr(query, alias, key, field);
    query.push(" ~ ");
    query.push_bind(IMPORT_PREVIEW_DECIMAL_NUMERIC_STRING_GRAMMAR);
    query.push(" AND ");
    push_preview_feedback_text_expr(query, alias, key, field);
    query.push(" !~ '^-' AND ");
    push_preview_feedback_text_expr(query, alias, key, field);
    query.push(" ~ ");
    query.push_bind(IMPORT_PREVIEW_DECIMAL_HAS_NON_ZERO);
    query.push(")");
}

fn push_preview_feedback_string_type_check(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
    key: &str,
    field: &str,
) {
    query.push("jsonb_typeof(");
    query.push(alias);
    query.push(".preview_payload#>'{preview_matching_feedback,");
    query.push(key);
    query.push(",");
    query.push(field);
    query.push("}') = 'string'");
}

fn push_preview_feedback_meaningful_text_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
    key: &str,
    field: &str,
) {
    query.push("(");
    push_preview_feedback_string_type_check(query, alias, key, field);
    query.push(" AND ");
    push_preview_feedback_text_expr(query, alias, key, field);
    query.push(" <> '' AND LOWER(");
    push_preview_feedback_text_expr(query, alias, key, field);
    query.push(") NOT IN ('none', 'suppressed', 'null') AND (NOT (");
    push_preview_feedback_text_expr(query, alias, key, field);
    query.push(" ~ ");
    query.push_bind(IMPORT_PREVIEW_DECIMAL_NUMERIC_STRING_GRAMMAR);
    query.push(") OR ");
    push_preview_feedback_positive_number_condition(query, alias, key, field);
    query.push("))");
}

fn push_sql_string_list(query: &mut QueryBuilder<'_, Postgres>, values: &[&str]) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            query.push(", ");
        }
        query.push("'");
        query.push(*value);
        query.push("'");
    }
}

fn push_preview_dedup_type_in_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
    values: &[&str],
) {
    query.push("LOWER(COALESCE(NULLIF(btrim(");
    query.push(alias);
    query.push(".preview_payload->>'dedup_type', ");
    query.push_bind(IMPORT_PREVIEW_SIGNAL_TRIM_CHARS);
    query.push("), ''), btrim(");
    query.push(alias);
    query.push(".preview_payload#>>'{preview_matching_feedback,dedup,type}', ");
    query.push_bind(IMPORT_PREVIEW_SIGNAL_TRIM_CHARS);
    query.push("), '')) IN (");
    push_sql_string_list(query, values);
    query.push(")");
}

fn push_preview_feedback_key_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
    key: &str,
) {
    query.push("(jsonb_typeof(");
    query.push(alias);
    query.push(".preview_payload->'preview_matching_feedback') = 'object' AND ");
    query.push(alias);
    query.push(".preview_payload->'preview_matching_feedback' ? '");
    query.push(key);
    query.push("')");
}

fn push_preview_feedback_family_text_search(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
    family: &str,
    status: &str,
) {
    let Some(key) = preview_feedback_key_for_signal_family(family) else {
        query.push("FALSE");
        return;
    };
    query.push("(");
    push_preview_feedback_key_condition(query, alias, key);
    query.push(" AND ");
    push_preview_feedback_resolved_status_expr(query, alias, key);
    query.push(" = ");
    query.push_bind(status.to_ascii_lowercase());
    query.push(")");
}

fn push_annotation_predicate(query: &mut QueryBuilder<'_, Postgres>, alias: &str, value: &str) {
    match value {
        "needs-review" => {
            query.push(" AND ");
            push_preview_current_review_condition(query, alias);
        }
        "no-issues" => {
            query.push(" AND NOT ");
            push_preview_current_review_condition(query, alias);
        }
        _ => {
            query.push(" AND ");
            query.push(alias);
            query.push(".preview_payload#>>'{preview_matching_feedback,annotation}' ILIKE ");
            query.push_bind(like_pattern(value));
        }
    }
}

fn push_preview_selection_target_predicates(
    query: &mut QueryBuilder<'_, Postgres>,
    target: ImportPreviewSelectionTarget,
    alias: &str,
) {
    match target {
        ImportPreviewSelectionTarget::All => {}
        ImportPreviewSelectionTarget::Valid => {
            query.push(" AND NOT ");
            push_preview_current_review_condition(query, alias);
        }
        ImportPreviewSelectionTarget::NeedsReview => {
            query.push(" AND ");
            push_preview_current_review_condition(query, alias);
        }
    }
}

fn push_preview_current_review_condition(query: &mut QueryBuilder<'_, Postgres>, alias: &str) {
    query.push("(");
    push_preview_missing_category_condition(query, alias);
    query.push(" OR ");
    push_preview_missing_source_account_condition(query, alias);
    query.push(" OR ");
    push_preview_missing_destination_account_condition(query, alias);
    query.push(" OR ");
    push_preview_same_transfer_accounts_condition(query, alias);
    query.push(" OR ");
    push_preview_identity_feedback_condition(query, alias);
    query.push(")");
}

fn push_preview_identity_feedback_condition(query: &mut QueryBuilder<'_, Postgres>, alias: &str) {
    query.push("COALESCE((jsonb_typeof(");
    query.push(alias);
    query.push(".preview_payload#>'{preview_matching_feedback,identity_validation,issues}') = 'array' AND jsonb_array_length(");
    query.push(alias);
    query.push(".preview_payload#>'{preview_matching_feedback,identity_validation,issues}') > 0), false)");
}

fn push_preview_missing_category_condition(query: &mut QueryBuilder<'_, Postgres>, alias: &str) {
    query.push("(");
    push_preview_category_required_type_condition(query, alias);
    query.push(" AND ");
    query.push("(");
    query.push(alias);
    query.push(".category_id IS NULL OR NOT EXISTS (SELECT 1 FROM categories c WHERE c.user_id = ");
    query.push(alias);
    query.push(".user_id AND c.id = ");
    query.push(alias);
    query.push(".category_id AND c.is_active = true) OR EXISTS (SELECT 1 FROM categories c WHERE c.user_id = ");
    query.push(alias);
    query.push(".user_id AND c.id = ");
    query.push(alias);
    query.push(".category_id AND c.is_active = true AND ");
    push_preview_category_type_mismatch_condition(query, alias, "c");
    query.push(")))");
}

fn push_preview_missing_source_account_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    query.push("(");
    query.push(alias);
    query.push(".account_id IS NULL OR NOT EXISTS (SELECT 1 FROM accounts a WHERE a.user_id = ");
    query.push(alias);
    query.push(".user_id AND a.id = ");
    query.push(alias);
    query.push(".account_id AND a.is_active = true))");
}

fn push_preview_missing_destination_account_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    query.push("(");
    push_preview_destination_account_type_condition(query, alias);
    query.push(" AND (");
    query.push(alias);
    query.push(".transfer_target_account_id IS NULL OR NOT EXISTS (SELECT 1 FROM accounts a WHERE a.user_id = ");
    query.push(alias);
    query.push(".user_id AND a.id = ");
    query.push(alias);
    query.push(".transfer_target_account_id AND a.is_active = true)))");
}

fn push_preview_same_transfer_accounts_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    query.push("(");
    push_preview_destination_account_type_condition(query, alias);
    query.push(" AND ");
    query.push(alias);
    query.push(".account_id IS NOT NULL AND ");
    query.push(alias);
    query.push(".transfer_target_account_id IS NOT NULL AND ");
    query.push(alias);
    query.push(".account_id = ");
    query.push(alias);
    query.push(".transfer_target_account_id)");
}

fn push_preview_category_required_type_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    query.push("lower(");
    query.push(alias);
    query.push(".transaction_type) IN ('收入', 'income', '2', '支出', 'expense', '3', '转账', 'transfer', '4', '投资', 'investment', '5')");
}

fn push_preview_destination_account_type_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    query.push("lower(");
    query.push(alias);
    query.push(".transaction_type) IN ('转账', 'transfer', '4', '投资', 'investment', '5')");
}

fn push_preview_category_type_mismatch_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    preview_alias: &str,
    category_alias: &str,
) {
    query.push("NOT (");
    query.push(category_alias);
    query.push(".category_type IS NULL OR trim(");
    query.push(category_alias);
    query.push(".category_type) = '' OR lower(");
    query.push(category_alias);
    query.push(".category_type) IN ('0', '1', 'all', '通用') OR (");
    push_preview_category_type_match_arm(
        query,
        preview_alias,
        category_alias,
        2,
        &["收入", "income", "2"],
    );
    query.push(") OR (");
    push_preview_category_type_match_arm(
        query,
        preview_alias,
        category_alias,
        3,
        &["支出", "expense", "3"],
    );
    query.push(") OR (");
    push_preview_category_type_match_arm(
        query,
        preview_alias,
        category_alias,
        4,
        &["转账", "transfer", "4"],
    );
    query.push(") OR (");
    push_preview_category_type_match_arm(
        query,
        preview_alias,
        category_alias,
        5,
        &["投资", "investment", "5"],
    );
    query.push("))");
}

fn push_preview_category_type_match_arm(
    query: &mut QueryBuilder<'_, Postgres>,
    preview_alias: &str,
    category_alias: &str,
    category_type: i64,
    preview_type_values: &[&str],
) {
    let category_alias = sanitize_sql_alias(category_alias);
    let preview_alias = sanitize_sql_alias(preview_alias);
    let category_name = match category_type {
        2 => "income",
        3 => "expense",
        4 => "transfer",
        5 => "investment",
        _ => "",
    };
    query.push(format!(
        "lower({category_alias}.category_type) IN ('{category_type}', '{category_name}') AND lower("
    ));
    query.push(preview_alias);
    query.push(".transaction_type) IN (");
    let mut preview_values = query.separated(", ");
    for value in preview_type_values {
        preview_values.push_bind((*value).to_string());
    }
    preview_values.push_unseparated(")");
}

fn sanitize_sql_alias(alias: &str) -> &str {
    match alias {
        "p" | "c" | "a" => alias,
        _ => "p",
    }
}

fn push_preview_account_filter_invalid_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    query.push("(");
    push_preview_missing_source_account_condition(query, alias);
    query.push(" OR ");
    push_preview_missing_destination_account_condition(query, alias);
    query.push(" OR ");
    push_preview_same_transfer_accounts_condition(query, alias);
    query.push(")");
}

fn push_preview_empty_parser_tags_condition(query: &mut QueryBuilder<'_, Postgres>, alias: &str) {
    query.push("(NOT EXISTS (SELECT 1 FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(");
    query.push(alias);
    query.push(".preview_payload->'preview_parser_tags') = 'array' THEN ");
    query.push(alias);
    query.push(".preview_payload->'preview_parser_tags' ELSE '[]'::jsonb END) AS parser_tag(value) WHERE btrim(parser_tag.value, ");
    query.push_bind(IMPORT_PREVIEW_SIGNAL_TRIM_CHARS);
    query.push(") <> ''))");
}
