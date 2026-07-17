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
    query.push(" IN (");
    push_sql_string_list(query, IMPORT_PREVIEW_TRANSFER_VISIBLE_STATUSES);
    query.push(") OR (");
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
    query.push("(");
    push_preview_meaningful_feedback_condition(
        query,
        alias,
        "learning",
        IMPORT_PREVIEW_LEARNING_NUMERIC_EVIDENCE_FIELDS,
        IMPORT_PREVIEW_LEARNING_TEXT_EVIDENCE_FIELDS,
    );
    query.push(" OR ");
    push_preview_transfer_learning_signal_condition(query, alias);
    query.push(")");
}

fn push_preview_transfer_learning_signal_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    query.push("(");
    push_preview_feedback_key_condition(query, alias, "transfer");
    query.push(" AND NOT ");
    push_preview_feedback_truthy_field_condition(query, alias, "transfer", "suppressed");
    query.push(" AND ");
    push_preview_feedback_string_type_check(query, alias, "transfer", "candidate_type");
    query.push(" AND ");
    push_preview_feedback_text_expr(query, alias, "transfer", "candidate_type");
    query.push(" <> '' AND ");
    push_preview_feedback_string_type_check(query, alias, "transfer", "learning_level");
    query.push(" AND ");
    push_preview_feedback_text_expr(query, alias, "transfer", "learning_level");
    query.push(" IN (");
    push_sql_string_list(query, IMPORT_PREVIEW_TRANSFER_LEARNING_LEVELS);
    query.push(") AND ");
    push_preview_feedback_resolved_status_expr(query, alias, "transfer");
    query.push(" <> '' AND ");
    push_preview_feedback_resolved_status_expr(query, alias, "transfer");
    query.push(" NOT IN (");
    push_sql_string_list(query, IMPORT_PREVIEW_SIGNAL_SUPPRESSED_STATUSES);
    query.push("))");
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
