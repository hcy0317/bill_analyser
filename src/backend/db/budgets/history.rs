fn enrich_budget_execution_history_items(
    items: Vec<BudgetRecord>,
    filters: &BudgetExecutionFilters,
    category_context: &bill_analyser_core::budgets::BudgetCategoryContext,
) -> Vec<BudgetRecord> {
    items
        .into_iter()
        .filter_map(|mut item| {
            let resolved_budget_type = resolve_budget_category_type(
                category_context,
                &record_text(&item, "category"),
                Some(&normalize_sub_category(item.get("sub_category"))),
                Some(filters.budget_type),
            )?;
            if resolved_budget_type.code() != filters.budget_type {
                return None;
            }
            let category_info = resolve_budget_category_info(
                category_context,
                &record_text(&item, "category"),
                Some(&normalize_sub_category(item.get("sub_category"))),
                Some(resolved_budget_type.code()),
            );
            item.insert(
                "type".to_string(),
                json_i64(i64::from(resolved_budget_type.code())),
            );
            item.insert(
                "category_id".to_string(),
                category_info
                    .get("id")
                    .and_then(value_to_i64)
                    .map(|id| Value::String(id.to_string()))
                    .unwrap_or_else(|| Value::String(String::new())),
            );
            item.insert("category_info".to_string(), category_info);
            Some(item)
        })
        .collect()
}

fn extract_exact_budget_history_items(
    items: &[BudgetRecord],
    start_date: &str,
    end_date: &str,
) -> Vec<BudgetRecord> {
    items
        .iter()
        .filter(|item| {
            record_text(item, "period_start") == start_date
                && record_text(item, "period_end") == end_date
        })
        .cloned()
        .collect()
}

fn budget_detail_overlaps_period(
    detail: &Value,
    period: &bill_analyser_core::budgets::BudgetPeriodRange,
) -> DbResult<bool> {
    let budget_start = value_field_string(detail, "start_date");
    if budget_start.trim().is_empty() {
        return Ok(true);
    }
    let budget_end = value_field_string(detail, "end_date");
    budget_overlaps_period(
        &budget_start,
        text_filter(Some(&budget_end)).as_deref(),
        &period.start_date,
        &period.end_date,
    )
    .map_err(DbError::InvalidOperation)
}

#[tracing::instrument(level = "debug", skip_all)]
fn merge_budget_execution_history_items(
    mut history_items: Vec<BudgetRecord>,
    on_demand_items: Vec<BudgetRecord>,
) -> Vec<BudgetRecord> {
    let mut history_keys = history_items
        .iter()
        .map(budget_history_identity_key)
        .collect::<BTreeSet<_>>();
    for item in on_demand_items {
        if history_keys.insert(budget_history_identity_key(&item)) {
            history_items.push(item);
        }
    }
    history_items
}

#[tracing::instrument(level = "debug", skip_all)]
fn sort_budget_execution_history_items(items: &mut [BudgetRecord]) {
    items.sort_by_key(|item| std::cmp::Reverse(budget_history_sort_key(item)));
}

fn budget_history_identity_key(item: &BudgetRecord) -> (i64, String, String) {
    (
        record_i64(item, "budget_id").unwrap_or_default(),
        record_text(item, "period_start"),
        record_text(item, "period_end"),
    )
}

fn budget_history_sort_key(item: &BudgetRecord) -> (String, String, String, String, i64) {
    (
        record_text(item, "period_start"),
        record_text(item, "period_end"),
        record_text(item, "category"),
        record_text(item, "sub_category"),
        record_i64(item, "budget_id").unwrap_or_default(),
    )
}

fn budget_history_filter_summary(filters: &BudgetExecutionFilters) -> String {
    build_budget_history_filter_summary(&BudgetHistoryFilterSummaryInput {
        budget_type: filters.budget_type,
        period_type: filters.period_type.clone(),
        budget_id: filters.budget_id,
        category_id: filters.category_id,
        account_ids: filters.account_ids.clone(),
        tag_ids: filters.tag_ids.clone(),
    })
}
