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

fn push_preview_current_review_condition_with_joins(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    query.push("((");
    push_preview_category_required_type_condition(query, alias);
    query.push(" AND (");
    query.push(alias);
    query.push(".category_id IS NULL OR c.id IS NULL OR ");
    push_preview_category_type_mismatch_condition(query, alias, "c");
    query.push(")) OR (");
    query.push(alias);
    query.push(".account_id IS NULL OR source_account.id IS NULL) OR (");
    push_preview_destination_account_type_condition(query, alias);
    query.push(" AND (");
    query.push(alias);
    query.push(".transfer_target_account_id IS NULL OR target_account.id IS NULL)) OR ");
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
