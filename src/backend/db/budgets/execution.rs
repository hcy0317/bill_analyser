
pub fn query_budget_execution_details(
    connection: &Connection,
    user_id: UserId,
    filters: &BudgetExecutionFilters,
) -> DbResult<Vec<Value>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let categories = load_category_context_values(connection, user_id)?;
    let category_context = build_budget_category_context(&categories);
    let category_filter = match filters.category_id {
        Some(category_id) => load_category_filter(connection, user_id, category_id)?,
        None => None,
    };
    if filters.category_id.is_some() && category_filter.is_none() {
        return Ok(Vec::new());
    }
    let mut budgets = query_budget_execution_candidates(connection, user_id, filters)?;
    if let Some((category, sub_category)) = category_filter {
        budgets.retain(|budget| {
            record_text(budget, "category") == category
                && (sub_category.is_empty()
                    || normalize_sub_category(budget.get("sub_category")) == sub_category)
        });
    }
    budgets = filter_budget_execution_candidates(budgets, filters, &category_context);
    budgets = dedupe_budget_execution_candidates(budgets);

    let type_name = get_budget_type_name(filters.budget_type);
    let mut results = Vec::new();
    for budget in budgets {
        let spent = get_budget_spent_amount(connection, user_id, &budget, type_name, filters)?;
        results.push(build_budget_execution_item(
            &budget,
            spent,
            &category_context,
            filters.budget_type,
        ));
    }
    Ok(results)
}

pub fn create_budget_execution_snapshots(
    connection: &mut Connection,
    user_id: UserId,
    filters: &BudgetExecutionFilters,
) -> DbResult<Value> {
    let user_id_value = UserScope::new(user_id).bind_value()?;
    let snapshots = query_budget_execution_details(connection, user_id, filters)?;
    let calculated_at = now_text();
    let filter_summary = budget_history_filter_summary(filters);
    let period_start = filters.start_date.clone().unwrap_or_default();
    let period_end = filters.end_date.clone().unwrap_or_default();

    run_transaction(connection, |tx| {
        let mut created_count = 0_i64;
        for snapshot in &snapshots {
            let budget_id = value_field_i64(snapshot, "id").unwrap_or_default();
            tx.execute(
                "
                DELETE FROM budget_history
                WHERE user_id = ?1 AND budget_id = ?2 AND period_start = ?3 AND period_end = ?4
                  AND filter_summary = ?5
                ",
                params![
                    user_id_value,
                    budget_id,
                    period_start.as_str(),
                    period_end.as_str(),
                    filter_summary.as_str()
                ],
            )?;

            let budget_amount = value_field_f64(snapshot, "budget_amount").unwrap_or_default();
            let spent_amount = value_field_f64(snapshot, "spent_amount").unwrap_or_default();
            let status = if spent_amount > budget_amount {
                "over_budget"
            } else {
                "within_budget"
            };
            tx.execute(
                "
                INSERT INTO budget_history (
                    user_id, budget_id, period_start, period_end,
                    budget_amount, spent_amount, remaining_amount,
                    execution_rate, status, filter_summary, calculated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                ",
                params![
                    user_id_value,
                    budget_id,
                    period_start.as_str(),
                    period_end.as_str(),
                    budget_amount,
                    spent_amount,
                    value_field_f64(snapshot, "remaining_amount").unwrap_or_default(),
                    value_field_f64(snapshot, "execution_rate").unwrap_or_default(),
                    status,
                    filter_summary.as_str(),
                    calculated_at.as_str()
                ],
            )?;
            created_count += 1;
        }

        Ok(json!({
            "created_count": created_count,
            "period_start": period_start,
            "period_end": period_end,
            "filter_summary": filter_summary,
            "calculated_at": calculated_at
        }))
    })
}
