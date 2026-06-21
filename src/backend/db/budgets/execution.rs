fn filter_budget_execution_candidates(
    budgets: Vec<BudgetRecord>,
    filters: &BudgetExecutionFilters,
    category_context: &bill_analyser_core::budgets::BudgetCategoryContext,
) -> Vec<BudgetRecord> {
    budgets
        .into_iter()
        .filter_map(|mut budget| {
            let resolved_type = resolve_budget_category_type(
                category_context,
                &record_text(&budget, "category"),
                Some(&normalize_sub_category(budget.get("sub_category"))),
                Some(filters.budget_type),
            )?;
            if resolved_type.code() != filters.budget_type {
                return None;
            }
            if !budget_overlaps_request_period(&budget, filters) {
                return None;
            }
            budget.insert(
                "_resolved_budget_type".to_string(),
                json_i64(i64::from(resolved_type.code())),
            );
            Some(budget)
        })
        .collect()
}

fn budget_overlaps_request_period(budget: &BudgetRecord, filters: &BudgetExecutionFilters) -> bool {
    let (Some(request_start), Some(request_end)) = (
        text_filter(filters.start_date.as_deref()),
        text_filter(filters.end_date.as_deref()),
    ) else {
        return true;
    };
    let budget_start = record_text(budget, "start_date");
    let budget_end = text_filter(Some(record_text(budget, "end_date").as_str()));
    if budget_start.trim().is_empty() {
        return true;
    }
    budget_start <= request_end && budget_end.is_none_or(|end| end >= request_start)
}

#[tracing::instrument(level = "debug", skip_all)]
fn dedupe_budget_execution_candidates(budgets: Vec<BudgetRecord>) -> Vec<BudgetRecord> {
    let mut selected: BTreeMap<(String, String, String, String, i64), BudgetRecord> =
        BTreeMap::new();
    let mut order = Vec::new();
    for budget in budgets {
        let category = record_text(&budget, "category").trim().to_string();
        let sub_category = normalize_sub_category(budget.get("sub_category"));
        let period_type = record_text(&budget, "period_type").trim().to_string();
        let start_date = record_text(&budget, "start_date").trim().to_string();
        let key = if category.is_empty() || period_type.is_empty() || start_date.is_empty() {
            (
                category,
                sub_category,
                period_type,
                start_date,
                record_i64(&budget, "id").unwrap_or_default(),
            )
        } else {
            (category, sub_category, period_type, start_date, 0)
        };
        let should_replace = selected.get(&key).is_none_or(|current| {
            (
                record_i64(&budget, "amount_cents").unwrap_or_default(),
                record_i64(&budget, "id").unwrap_or_default(),
            ) >= (
                record_i64(current, "amount_cents").unwrap_or_default(),
                record_i64(current, "id").unwrap_or_default(),
            )
        });
        if !selected.contains_key(&key) {
            order.push(key.clone());
        }
        if should_replace {
            selected.insert(key, budget);
        }
    }
    order
        .into_iter()
        .filter_map(|key| selected.remove(&key))
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
fn resolve_budget_execution_window(
    budget: &BudgetRecord,
    filters: &BudgetExecutionFilters,
) -> (Option<String>, Option<String>) {
    let budget_defined_start = text_filter(Some(record_text(budget, "start_date").as_str()));
    let budget_defined_end = text_filter(Some(record_text(budget, "end_date").as_str()));
    let mut budget_start = filters.start_date.clone().or(budget_defined_start.clone());
    let mut budget_end = filters.end_date.clone().or(budget_defined_end.clone());
    if let (Some(request_start), Some(defined_start)) =
        (filters.start_date.as_ref(), budget_defined_start.as_ref())
    {
        budget_start = Some(request_start.max(defined_start).clone());
    }
    if let (Some(request_end), Some(defined_end)) =
        (filters.end_date.as_ref(), budget_defined_end.as_ref())
    {
        budget_end = Some(request_end.min(defined_end).clone());
    }
    (
        budget_start,
        normalize_budget_query_end_date(budget_end.as_deref()),
    )
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_budget_execution_item(
    budget: &BudgetRecord,
    spent_cents: i64,
    category_context: &bill_analyser_core::budgets::BudgetCategoryContext,
    fallback_budget_type: i32,
) -> Value {
    let budget_amount_cents = record_i64(budget, "amount_cents").unwrap_or_default();
    let resolved_budget_type = record_i64(budget, "_resolved_budget_type")
        .and_then(|value| i32::try_from(value).ok())
        .unwrap_or(fallback_budget_type);
    let category = record_text(budget, "category");
    let sub_category = normalize_sub_category(budget.get("sub_category"));
    let category_info = resolve_budget_category_info(
        category_context,
        &category,
        Some(&sub_category),
        Some(resolved_budget_type),
    );
    let category_id = category_info
        .get("id")
        .and_then(value_to_i64)
        .map(|id| id.to_string())
        .unwrap_or_default();
    let execution_rate = if budget_amount_cents > 0 {
        round2((spent_cents as f64 / budget_amount_cents as f64) * 100.0)
    } else {
        0.0
    };
    json!({
        "id": record_i64(budget, "id").unwrap_or_default(),
        "name": record_text(budget, "name"),
        "category": category,
        "sub_category": sub_category,
        "category_info": category_info,
        "category_id": category_id,
        "period_type": record_text(budget, "period_type"),
        "budget_amount_cents": budget_amount_cents,
        "spent_amount_cents": spent_cents,
        "remaining_amount_cents": budget_amount_cents - spent_cents,
        "execution_rate": execution_rate,
        "type": resolved_budget_type,
        "alert_threshold": record_i64(budget, "alert_threshold").unwrap_or(80),
        "start_date": field_or_null(budget, "start_date"),
        "end_date": field_or_null(budget, "end_date"),
        "enabled": record_i64(budget, "enabled").unwrap_or(1)
    })
}
