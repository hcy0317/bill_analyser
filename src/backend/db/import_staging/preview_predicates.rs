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
        "parser" => {
            query.push("(");
            push_preview_parser_signal_condition(query, alias);
            query.push(" AND NOT (");
            push_preview_specific_visible_signal_condition(query, alias);
            query.push("))");
        }
        "platform_duplicate" => push_preview_platform_duplicate_signal_condition(query, alias),
        "transfer" => push_preview_transfer_signal_condition(query, alias),
        "history" => push_preview_history_signal_condition(query, alias),
        "learning" => push_preview_recommendation_signal_condition(query, alias),
        "llm" => push_preview_feedback_key_condition(query, alias, "llm"),
        _ => {
            query.push("FALSE");
        }
    }
}

fn push_preview_parser_signal_condition(query: &mut QueryBuilder<'_, Postgres>, alias: &str) {
    query.push("(COALESCE(NULLIF(");
    query.push(alias);
    query.push(".preview_payload->>'preview_parser_id', ''), '') <> '' OR COALESCE(jsonb_array_length(CASE WHEN jsonb_typeof(");
    query.push(alias);
    query.push(".preview_payload->'preview_parser_tags') = 'array' THEN ");
    query.push(alias);
    query.push(".preview_payload->'preview_parser_tags' ELSE '[]'::jsonb END), 0) > 0 OR ");
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
    push_preview_feedback_key_condition(query, alias, "llm");
}

fn push_preview_platform_duplicate_signal_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    push_preview_dedup_type_in_condition(query, alias, &["platform_bank"]);
}

fn push_preview_transfer_signal_condition(query: &mut QueryBuilder<'_, Postgres>, alias: &str) {
    query.push("(");
    push_preview_feedback_key_condition(query, alias, "transfer");
    query.push(" AND (");
    query.push("LOWER(COALESCE(");
    query.push(alias);
    query.push(".preview_payload#>>'{preview_matching_feedback,transfer,review_status}', '')) IN ('accepted', 'rejected', 'skipped') OR (LOWER(COALESCE(");
    query.push(alias);
    query.push(".preview_payload#>>'{preview_matching_feedback,transfer,review_status}', '')) = 'pending' AND LOWER(COALESCE(");
    query.push(alias);
    query.push(".preview_payload#>>'{preview_matching_feedback,transfer,suppressed}', 'false')) <> 'true') OR (LOWER(COALESCE(");
    query.push(alias);
    query.push(".preview_payload#>>'{preview_matching_feedback,transfer,suppressed}', 'false')) <> 'true' AND LOWER(COALESCE(");
    query.push(alias);
    query.push(".preview_payload->>'preview_type', '')) NOT IN ('转账', 'transfer', '4') AND (COALESCE(NULLIF(");
    query.push(alias);
    query.push(".preview_payload#>>'{preview_matching_feedback,transfer,candidate_type}', ''), '') <> '' OR COALESCE(NULLIF(");
    query.push(alias);
    query.push(
        ".preview_payload#>>'{preview_matching_feedback,transfer,reason}', ''), '') <> ''))))",
    );
}

fn push_preview_recommendation_signal_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    push_preview_feedback_key_condition(query, alias, "learning");
}

fn push_preview_history_signal_condition(query: &mut QueryBuilder<'_, Postgres>, alias: &str) {
    query.push("(LOWER(COALESCE(");
    query.push(alias);
    query.push(".preview_payload#>>'{preview_matching_feedback,reconciliation,planned_operation}', '')) IN ('update_history', 'merge_transfer_history') OR LOWER(COALESCE(");
    query.push(alias);
    query.push(".preview_payload#>>'{preview_matching_feedback,reconciliation,destructive_ack_required}', '')) = 'true')");
}

fn push_preview_dedup_type_in_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
    values: &[&str],
) {
    query.push("LOWER(COALESCE(");
    query.push(alias);
    query.push(".preview_payload->>'dedup_type', ");
    query.push(alias);
    query.push(".preview_payload#>>'{preview_matching_feedback,dedup,type}', '')) IN (");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            query.push(", ");
        }
        query.push("'");
        query.push(*value);
        query.push("'");
    }
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
    push_preview_feedback_text_search_condition(query, alias, key, status);
}

fn push_preview_feedback_text_search_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
    key: &str,
    status: &str,
) {
    query.push("(");
    push_preview_feedback_key_condition(query, alias, key);
    query.push(" AND (");
    query.push(alias);
    query.push(".preview_payload#>'{preview_matching_feedback,");
    query.push(key);
    query.push("}')::text ILIKE ");
    query.push_bind(like_pattern(status));
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
    query.push("(jsonb_typeof(");
    query.push(alias);
    query.push(".preview_payload#>'{preview_matching_feedback,identity_validation,issues}') = 'array' AND jsonb_array_length(");
    query.push(alias);
    query.push(".preview_payload#>'{preview_matching_feedback,identity_validation,issues}') > 0)");
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
    query.push("(COALESCE(jsonb_array_length(CASE WHEN jsonb_typeof(");
    query.push(alias);
    query.push(".preview_payload->'preview_parser_tags') = 'array' THEN ");
    query.push(alias);
    query.push(".preview_payload->'preview_parser_tags' ELSE '[]'::jsonb END), 0) = 0)");
}
