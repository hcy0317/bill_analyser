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

fn push_preview_unknown_signal_status_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    query.push("(");
    for (index, (family, canonical_statuses)) in [
        ("transfer", IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES),
        ("reconciliation", IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES),
        ("learning", IMPORT_PREVIEW_LEARNING_CANONICAL_STATUSES),
        ("llm", IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES),
    ]
    .into_iter()
    .enumerate()
    {
        if index > 0 {
            query.push(" OR ");
        }
        query.push("(");
        push_preview_feedback_resolved_status_expr(query, alias, family);
        query.push(" <> '' AND ");
        push_preview_feedback_resolved_status_expr(query, alias, family);
        query.push(" NOT IN (");
        push_sql_string_list(query, canonical_statuses);
        query.push("))");
    }
    query.push(")");
}

fn push_preview_matching_feedback_resolved_status_expr(
    query: &mut QueryBuilder<'_, Postgres>,
    matching_feedback_expr: &str,
    key: &str,
) {
    let mut first = true;
    query.push("COALESCE(");
    for status_field in IMPORT_PREVIEW_SIGNAL_STATUS_FIELDS {
        if !first {
            query.push(", ");
        }
        query.push("NULLIF(LOWER(btrim(COALESCE(");
        query.push(matching_feedback_expr);
        query.push("#>>'{");
        query.push(key);
        query.push(",");
        query.push(*status_field);
        query.push("}', ''), ");
        query.push_bind(IMPORT_PREVIEW_SIGNAL_TRIM_CHARS);
        query.push(")), '')");
        first = false;
    }
    query.push(", '')");
}

fn push_preview_unknown_signal_status_from_matching_feedback_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    matching_feedback_expr: &str,
) {
    query.push("(");
    for (index, (family, canonical_statuses)) in [
        ("transfer", IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES),
        ("reconciliation", IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES),
        ("learning", IMPORT_PREVIEW_LEARNING_CANONICAL_STATUSES),
        ("llm", IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES),
    ]
    .into_iter()
    .enumerate()
    {
        if index > 0 {
            query.push(" OR ");
        }
        query.push("(");
        push_preview_matching_feedback_resolved_status_expr(
            query,
            matching_feedback_expr,
            family,
        );
        query.push(" <> '' AND ");
        push_preview_matching_feedback_resolved_status_expr(
            query,
            matching_feedback_expr,
            family,
        );
        query.push(" NOT IN (");
        push_sql_string_list(query, canonical_statuses);
        query.push("))");
    }
    query.push(")");
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
