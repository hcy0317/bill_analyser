fn apply_preview_filters(
    rows: Vec<ImportPreviewRow>,
    filters: &ImportPreviewQueryFilters,
) -> Vec<ImportPreviewRow> {
    rows.into_iter()
        .filter(|row| {
            datetime_filter_matches(
                filters.min_datetime.as_deref(),
                filters.max_datetime.as_deref(),
                row,
            ) && selected_filter_matches(filters.selected_only, row)
                && text_filter_matches(filters.transaction_type.as_deref(), &row.preview_type)
                && category_filter_matches(filters.category.as_deref(), row)
                && account_filter_matches(filters.account.as_deref(), row)
                && tag_filter_matches(filters.tag.as_deref(), row)
                && signal_filter_matches(filters.signal.as_deref(), row)
                && annotation_filter_matches(filters.annotation.as_deref(), row)
                && text_filter_matches(filters.description.as_deref(), &row.preview_description)
        })
        .collect()
}

pub fn get_import_preview_category_by_id(
    pool: &PostgresPool,
    user_id: UserId,
    category_id: i64,
) -> DbResult<Option<ImportPreviewCategoryLookup>> {
    block_on_db(async move {
        let row = sqlx::query(import_preview_category_lookup_sql())
            .bind(category_id)
            .bind(user_id_i64(user_id)?)
            .fetch_optional(pool)
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let name = row.try_get::<String, _>("name")?;
        let path = row
            .try_get::<Option<String>, _>("path")?
            .unwrap_or_default();
        let category_type = row.try_get::<Option<String>, _>("category_type")?;
        Ok(Some(import_preview_category_lookup_from_values(
            &name,
            &path,
            category_type.as_deref(),
        )))
    })
}

pub fn get_import_preview_categories_by_ids(
    pool: &PostgresPool,
    user_id: UserId,
    category_ids: &[i64],
) -> DbResult<HashMap<i64, ImportPreviewCategoryLookup>> {
    block_on_db(async move {
        if category_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut query = QueryBuilder::<Postgres>::new(
            "SELECT id, name, category_type, path FROM categories WHERE user_id = ",
        );
        query.push_bind(user_id_i64(user_id)?);
        query.push(" AND is_active = true AND id IN (");
        let mut separated = query.separated(", ");
        for category_id in category_ids {
            separated.push_bind(category_id);
        }
        separated.push_unseparated(")");
        let rows = query.build().fetch_all(pool).await?;
        rows.into_iter()
            .map(|row| {
                let category_id = row.try_get::<i64, _>("id")?;
                let name = row.try_get::<String, _>("name")?;
                let path = row
                    .try_get::<Option<String>, _>("path")?
                    .unwrap_or_default();
                let category_type = row.try_get::<Option<String>, _>("category_type")?;
                Ok((
                    category_id,
                    import_preview_category_lookup_from_values(
                        &name,
                        &path,
                        category_type.as_deref(),
                    ),
                ))
            })
            .collect()
    })
}

fn import_preview_category_lookup_sql() -> &'static str {
    r#"
        SELECT name, category_type, path
        FROM categories
        WHERE id = $1 AND user_id = $2 AND is_active = true
    "#
    .trim()
}

fn import_preview_category_lookup_from_values(
    name: &str,
    path: &str,
    category_type: Option<&str>,
) -> ImportPreviewCategoryLookup {
    let (main_category, sub_category) =
        category_names_from_postgres_path(Some(path), name);
    let type_code = category_type.and_then(preview_category_type_code);
    ImportPreviewCategoryLookup {
        type_code,
        main_category,
        sub_category,
    }
}

fn preview_category_type_code(value: &str) -> Option<i64> {
    match value.trim().to_ascii_lowercase().as_str() {
        "2" | "income" | "收入" => Some(2),
        "3" | "expense" | "支出" => Some(3),
        "4" | "transfer" | "转账" => Some(4),
        "5" | "investment" | "投资" => Some(5),
        _ => None,
    }
}

fn datetime_filter_matches(
    min_datetime: Option<&str>,
    max_datetime: Option<&str>,
    row: &ImportPreviewRow,
) -> bool {
    let preview_date = normalize_bill_date_text(&row.preview_date);
    min_datetime
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none_or(|value| preview_date.as_str() >= value)
        && max_datetime
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none_or(|value| preview_date.as_str() <= value)
}

fn selected_filter_matches(selected_only: bool, row: &ImportPreviewRow) -> bool {
    !selected_only || row.preview_selected
}

fn category_filter_matches(filter: Option<&str>, row: &ImportPreviewRow) -> bool {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    if filter == "__none__" {
        return row.category_id.is_none()
            && row.preview_main_category.trim().is_empty()
            && row.preview_sub_category.trim().is_empty();
    }
    if filter == "__invalid__" {
        return preview_has_missing_category_issue(row)
            || preview_identity_issue_matches_field(row, "category_id");
    }
    row.category_id
        .is_some_and(|category_id| category_id.to_string() == filter)
}

fn account_filter_matches(filter: Option<&str>, row: &ImportPreviewRow) -> bool {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    if filter == "__none__" {
        return row.preview_source_account_id.is_none()
            && row.preview_destination_account_id.is_none()
            && row.preview_payment_method.trim().is_empty();
    }
    if filter == "__invalid__" {
        return preview_account_filter_invalid_matches(row);
    }
    let source = row.preview_source_account_id.map(|value| value.to_string());
    let destination = row
        .preview_destination_account_id
        .map(|value| value.to_string());
    source.as_deref().is_some_and(|value| value == filter)
        || destination.as_deref().is_some_and(|value| value == filter)
}

fn text_filter_matches(filter: Option<&str>, value: &str) -> bool {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    value.to_lowercase().contains(&filter.to_lowercase())
}

fn tag_filter_matches(filter: Option<&str>, row: &ImportPreviewRow) -> bool {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    if matches!(filter, "__none__" | "__invalid__") {
        return row.preview_parser_tags.is_empty();
    }
    row.preview_parser_tags
        .iter()
        .any(|tag| tag.eq_ignore_ascii_case(filter) || text_filter_matches(Some(filter), tag))
}

fn signal_filter_matches(filter: Option<&str>, row: &ImportPreviewRow) -> bool {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    let filter = normalize_visible_signal_filter(filter);
    let Some((family, status)) = signal_filter_family_status(&filter) else {
        return preview_signal_family_matches(&filter, row);
    };
    preview_signal_family_matches(family, row)
        && preview_feedback_family_contains_status(&row.preview_matching_feedback, family, status)
}

fn annotation_filter_matches(filter: Option<&str>, row: &ImportPreviewRow) -> bool {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    if filter == "needs-review" {
        return preview_requires_review(row);
    }
    if filter == "no-issues" {
        return !preview_requires_review(row);
    }
    let filter = filter.to_ascii_lowercase();
    row.preview_matching_feedback
        .get("annotation")
        .is_some_and(|value| json_value_contains_text(value, &filter))
}

fn normalize_visible_signal_filter(filter: &str) -> String {
    let normalized = trim_import_preview_signal_text(filter).to_ascii_lowercase();
    if normalized.contains(':') {
        return normalized;
    }
    strip_numeric_visible_signal_suffix(&normalized).to_string()
}

fn strip_numeric_visible_signal_suffix(filter: &str) -> &str {
    for delimiter in ['_', '-'] {
        if let Some((family, suffix)) = filter.rsplit_once(delimiter) {
            if suffix.chars().all(|ch| ch.is_ascii_digit()) && is_visible_signal_family(family) {
                return family;
            }
        }
    }
    filter
}

fn signal_filter_family_status(filter: &str) -> Option<(&str, &str)> {
    let (family, status) = filter.split_once(':')?;
    let family = trim_import_preview_signal_text(family);
    let status = trim_import_preview_signal_text(status);
    if ImportPreviewSignalFamily::parse(family).is_none() || status.is_empty() {
        return None;
    }
    Some((family, status))
}

fn preview_signal_family_matches(family: &str, row: &ImportPreviewRow) -> bool {
    match ImportPreviewSignalFamily::parse(family) {
        Some(ImportPreviewSignalFamily::Parser) => {
            preview_parser_signal_matches(row) && !preview_has_specific_visible_signal(row)
        }
        Some(ImportPreviewSignalFamily::PlatformDuplicate) => {
            preview_platform_duplicate_signal_matches(row)
        }
        Some(ImportPreviewSignalFamily::Transfer) => preview_transfer_signal_matches(row),
        Some(ImportPreviewSignalFamily::History) => preview_history_signal_matches(row),
        Some(ImportPreviewSignalFamily::Learning) => preview_recommendation_signal_matches(row),
        Some(ImportPreviewSignalFamily::Llm) => {
            import_preview_recommendation_feedback_family_is_meaningful(
                &row.preview_matching_feedback,
                "llm",
            )
        }
        _ => false,
    }
}

fn preview_parser_signal_matches(row: &ImportPreviewRow) -> bool {
    !trim_import_preview_signal_text(&row.preview_parser_id).is_empty()
        || row
            .preview_parser_tags
            .iter()
            .any(|tag| !trim_import_preview_signal_text(tag).is_empty())
        || preview_feedback_key_exists(&row.preview_matching_feedback, "parser")
}

fn preview_has_specific_visible_signal(row: &ImportPreviewRow) -> bool {
    preview_platform_duplicate_signal_matches(row)
        || preview_transfer_signal_matches(row)
        || preview_history_signal_matches(row)
        || preview_recommendation_signal_matches(row)
        || import_preview_recommendation_feedback_family_is_meaningful(
            &row.preview_matching_feedback,
            "llm",
        )
}

fn preview_platform_duplicate_signal_matches(row: &ImportPreviewRow) -> bool {
    preview_dedup_type(row) == "platform_bank"
}

fn preview_transfer_signal_matches(row: &ImportPreviewRow) -> bool {
    let Some(transfer) = row
        .preview_matching_feedback
        .get("transfer")
        .and_then(Value::as_object)
    else {
        return false;
    };
    // Suppressed check first
    let suppressed = transfer
        .get("suppressed")
        .map(import_preview_signal_value_is_truthy)
        .unwrap_or(false);
    if suppressed {
        return false;
    }
    let resolved_status = resolve_first_nonempty_status(transfer);
    if matches!(resolved_status.as_str(), "none" | "suppressed") {
        return false;
    }
    if IMPORT_PREVIEW_TRANSFER_VISIBLE_STATUSES.contains(&resolved_status.as_str()) {
        return true;
    }
    if !resolved_status.is_empty() {
        return false;
    }
    let has_candidate = transfer
        .get("candidate_type")
        .and_then(Value::as_str)
        .is_some_and(|value| !trim_import_preview_signal_text(value).is_empty())
        || transfer
            .get("reason")
            .and_then(Value::as_str)
            .is_some_and(|value| !trim_import_preview_signal_text(value).is_empty())
        || transfer.get("score").is_some_and(|value| match value {
            Value::Number(number) => number.as_f64().is_some_and(|value| value > 0.0),
            Value::String(text) => strict_decimal_is_positive(text),
            _ => false,
        });
    has_candidate
        && !matches!(
            row.preview_type.trim().to_ascii_lowercase().as_str(),
            "转账" | "transfer" | "4"
        )
}

fn preview_recommendation_signal_matches(row: &ImportPreviewRow) -> bool {
    import_preview_recommendation_feedback_family_is_meaningful(
        &row.preview_matching_feedback,
        "learning",
    ) || preview_transfer_learning_signal_matches(row)
}

fn preview_transfer_learning_signal_matches(row: &ImportPreviewRow) -> bool {
    let Some(transfer) = row
        .preview_matching_feedback
        .get("transfer")
        .and_then(Value::as_object)
    else {
        return false;
    };
    if transfer
        .get("suppressed")
        .map(import_preview_signal_value_is_truthy)
        .unwrap_or(false)
    {
        return false;
    }
    let learning_level = transfer
        .get("learning_level")
        .and_then(Value::as_str)
        .map(trim_import_preview_signal_text)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !IMPORT_PREVIEW_TRANSFER_LEARNING_LEVELS.contains(&learning_level.as_str()) {
        return false;
    }
    let has_candidate = transfer
        .get("candidate_type")
        .and_then(Value::as_str)
        .is_some_and(|value| !trim_import_preview_signal_text(value).is_empty());
    let status = resolve_first_nonempty_status(transfer);
    has_candidate
        && !status.is_empty()
        && IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES.contains(&status.as_str())
        && !IMPORT_PREVIEW_SIGNAL_SUPPRESSED_STATUSES.contains(&status.as_str())
}

fn preview_history_signal_matches(row: &ImportPreviewRow) -> bool {
    let reconciliation = row.preview_matching_feedback.get("reconciliation");
    if reconciliation
        .and_then(Value::as_object)
        .is_some_and(|section| {
            let status = resolve_first_nonempty_status(section);
            !status.is_empty()
                && !IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES.contains(&status.as_str())
        })
    {
        return false;
    }
    let planned_operation = reconciliation
        .and_then(|value| value.get("planned_operation"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    normalize_history_operation(&planned_operation).is_some()
        || reconciliation
            .and_then(|value| value.get("destructive_ack_required"))
            .map(import_preview_signal_value_is_truthy)
            .unwrap_or(false)
}

fn preview_dedup_type(row: &ImportPreviewRow) -> String {
    let direct = trim_import_preview_signal_text(&row.dedup_type);
    if !direct.is_empty() {
        return direct.to_ascii_lowercase();
    }
    row.preview_matching_feedback
        .pointer("/dedup/type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim_matches(|ch| IMPORT_PREVIEW_SIGNAL_TRIM_CHARS.contains(ch))
        .to_ascii_lowercase()
}

fn preview_feedback_key_exists(feedback: &Value, key: &str) -> bool {
    feedback
        .as_object()
        .is_some_and(|object| object.contains_key(key))
}

fn preview_feedback_family_contains_status(feedback: &Value, family: &str, status: &str) -> bool {
    preview_feedback_key_for_signal_family(family).is_some_and(|key| {
        feedback.get(key).is_some_and(|value| {
            value
                .as_object()
                .map(resolve_first_nonempty_status)
                .is_some_and(|resolved| resolved == status)
        })
    })
}

fn preview_feedback_key_for_signal_family(family: &str) -> Option<&'static str> {
    match family {
        "parser" => Some("parser"),
        "platform_duplicate" => Some("dedup"),
        "transfer" => Some("transfer"),
        "history" => Some("reconciliation"),
        "learning" => Some("learning"),
        "llm" => Some("llm"),
        _ => None,
    }
}

fn is_visible_signal_family(family: &str) -> bool {
    is_import_preview_visible_signal_family(family)
}

fn json_value_contains_text(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(text) => text.to_ascii_lowercase().contains(needle),
        Value::Array(items) => items
            .iter()
            .any(|item| json_value_contains_text(item, needle)),
        Value::Object(object) => object
            .values()
            .any(|item| json_value_contains_text(item, needle)),
        Value::Number(number) => number.to_string().contains(needle),
        Value::Bool(value) => value.to_string().contains(needle),
        Value::Null => false,
    }
}

fn sort_preview_rows(rows: &mut [ImportPreviewRow], sort_by: &str, sort_direction: &str) {
    let descending = sort_direction.eq_ignore_ascii_case("desc");
    rows.sort_by(|left, right| {
        let order = match sort_by {
            "amount_cents"
            | "preview_amount_cents"
            | "previewAmountCents"
            | "sourceAmountCents" => left
                .preview_amount_cents
                .cmp(&right.preview_amount_cents)
                .then(left.id.cmp(&right.id)),
            "counterparty" => left
                .preview_counterparty
                .cmp(&right.preview_counterparty)
                .then(left.id.cmp(&right.id)),
            "type" => preview_type_sort_rank(&left.preview_type)
                .cmp(&preview_type_sort_rank(&right.preview_type))
                .then(left.preview_type.cmp(&right.preview_type))
                .then(left.id.cmp(&right.id)),
            "paymentMethod" => left
                .preview_payment_method
                .cmp(&right.preview_payment_method)
                .then(left.id.cmp(&right.id)),
            "comment" => left
                .preview_description
                .cmp(&right.preview_description)
                .then(left.id.cmp(&right.id)),
            "time" => left
                .preview_date
                .cmp(&right.preview_date)
                .then(left.id.cmp(&right.id)),
            _ => left
                .preview_date
                .cmp(&right.preview_date)
                .then(left.id.cmp(&right.id)),
        };
        if descending {
            order.reverse()
        } else {
            order
        }
    });
}

fn preview_type_sort_rank(value: &str) -> u8 {
    match value.trim().to_ascii_lowercase().as_str() {
        "收入" | "income" | "2" => 0,
        "支出" | "expense" | "3" => 1,
        "转账" | "transfer" | "4" => 2,
        "投资" | "investment" | "5" => 3,
        _ => 4,
    }
}

fn preview_requires_review(preview: &ImportPreviewRow) -> bool {
    preview_has_missing_category_issue(preview)
        || preview_has_missing_source_account_issue(preview)
        || preview_has_missing_destination_account_issue(preview)
        || preview_has_same_transfer_accounts_issue(preview)
        || preview_has_identity_validation_issue(preview)
        || import_preview_matching_feedback_has_unknown_signal_status(
            &preview.preview_matching_feedback,
        )
}

fn preview_has_missing_category_issue(preview: &ImportPreviewRow) -> bool {
    preview_category_required(&preview.preview_type) && preview.category_id.is_none()
}

fn preview_has_missing_source_account_issue(preview: &ImportPreviewRow) -> bool {
    preview.preview_source_account_id.is_none()
}

fn preview_has_missing_destination_account_issue(preview: &ImportPreviewRow) -> bool {
    preview_destination_account_required(&preview.preview_type)
        && preview.preview_destination_account_id.is_none()
}

fn preview_has_same_transfer_accounts_issue(preview: &ImportPreviewRow) -> bool {
    preview_destination_account_required(&preview.preview_type)
        && preview.preview_source_account_id.is_some()
        && preview.preview_destination_account_id.is_some()
        && preview.preview_source_account_id == preview.preview_destination_account_id
}

fn preview_has_identity_validation_issue(preview: &ImportPreviewRow) -> bool {
    preview
        .preview_matching_feedback
        .pointer("/identity_validation/issues")
        .and_then(Value::as_array)
        .is_some_and(|issues| !issues.is_empty())
}

fn preview_account_filter_invalid_matches(preview: &ImportPreviewRow) -> bool {
    preview_has_missing_source_account_issue(preview)
        || preview_has_missing_destination_account_issue(preview)
        || preview_has_same_transfer_accounts_issue(preview)
        || preview_identity_issue_matches_field(preview, "source_account_id")
        || preview_identity_issue_matches_field(preview, "destination_account_id")
}

fn preview_identity_issue_matches_field(preview: &ImportPreviewRow, expected_field: &str) -> bool {
    preview
        .preview_matching_feedback
        .pointer("/identity_validation/issues")
        .and_then(Value::as_array)
        .is_some_and(|issues| {
            issues.iter().any(|issue| {
                issue
                    .get("field")
                    .and_then(Value::as_str)
                    .is_some_and(|field| field == expected_field)
            })
        })
}

fn preview_category_required(preview_type: &str) -> bool {
    matches!(
        preview_type.trim().to_ascii_lowercase().as_str(),
        "收入"
            | "income"
            | "2"
            | "支出"
            | "expense"
            | "3"
            | "转账"
            | "transfer"
            | "4"
            | "投资"
            | "investment"
            | "5"
    )
}

fn preview_destination_account_required(preview_type: &str) -> bool {
    matches!(
        preview_type.trim().to_ascii_lowercase().as_str(),
        "转账" | "transfer" | "4" | "投资" | "investment" | "5"
    )
}
