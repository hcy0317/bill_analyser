async fn query_postgres_budget_execution_candidates(
    pool: &PostgresPool,
    user_id: i64,
    filters: &BudgetExecutionFilters,
) -> DbResult<Vec<BudgetRecord>> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        SELECT id, user_id, name, category, sub_category, period_type,
            amount_cents, start_date, end_date, alert_threshold, enabled,
            created_at, updated_at
        FROM budgets
        WHERE user_id =
        "#,
    );
    builder.push_bind(user_id);
    builder.push(" AND enabled = true");
    if let Some(period_type) = text_filter(filters.period_type.as_deref()) {
        builder.push(" AND period_type = ");
        builder.push_bind(period_type);
    }
    if let Some(budget_id) = filters.budget_id {
        builder.push(" AND id = ");
        builder.push_bind(budget_id);
    }
    builder.push(" ORDER BY created_at DESC, id DESC");
    let rows = builder.build().fetch_all(pool).await?;
    rows.into_iter()
        .map(budget_record_from_postgres_row)
        .collect()
}

async fn get_postgres_budget_spent_amount(
    pool: &PostgresPool,
    user_id: i64,
    budget: &BudgetRecord,
    type_name: &str,
    filters: &BudgetExecutionFilters,
) -> DbResult<i64> {
    let mut builder = QueryBuilder::<Postgres>::new(
        "SELECT COALESCE(SUM(ABS(amount_cents)), 0)::BIGINT AS spent_cents FROM bills b LEFT JOIN categories c ON c.user_id = b.user_id AND c.id = b.category_id WHERE b.user_id = ",
    );
    builder.push_bind(user_id);
    builder.push(" AND b.is_deleted = false");
    builder.push(" AND (b.transaction_type = ");
    builder.push_bind(canonical_budget_transaction_type(type_name));
    builder.push(" OR b.transaction_type = ");
    builder.push_bind(type_name.to_string());
    builder.push(" OR b.standard_payload->>'type' = ");
    builder.push_bind(type_name.to_string());
    builder.push(")");
    if let Some(category) = text_filter(Some(record_text(budget, "category").as_str())) {
        builder.push(" AND ");
        push_postgres_bill_main_category_expr(&mut builder);
        builder.push(" = ");
        builder.push_bind(category);
    }
    if let Some(sub_category) = text_filter(Some(record_text(budget, "sub_category").as_str())) {
        builder.push(" AND ");
        push_postgres_bill_sub_category_expr(&mut builder);
        builder.push(" = ");
        builder.push_bind(sub_category);
    }
    let (budget_start, budget_end) = super::resolve_budget_execution_window(budget, filters);
    if let Some(start_date) = budget_start {
        builder.push(" AND b.occurred_at >= ");
        builder.push_bind(start_date);
        builder.push("::date");
    }
    if let Some(end_date) = budget_end {
        builder.push(" AND b.occurred_at < (");
        builder.push_bind(end_date);
        builder.push("::date + interval '1 day')");
    }
    if let Some(account_ids) = filters.account_ids.as_deref().filter(|ids| !ids.is_empty()) {
        builder.push(" AND (b.account_id IN (");
        push_postgres_i64_bind_list(&mut builder, account_ids);
        builder.push(") OR b.source_account_id IN (");
        push_postgres_i64_bind_list(&mut builder, account_ids);
        builder.push(") OR b.target_account_id IN (");
        push_postgres_i64_bind_list(&mut builder, account_ids);
        builder.push(") OR b.transfer_target_account_id IN (");
        push_postgres_i64_bind_list(&mut builder, account_ids);
        builder.push("))");
    }
    if let Some(tag_ids) = filters.tag_ids.as_deref().filter(|ids| !ids.is_empty()) {
        builder.push(" AND b.id IN (SELECT bill_id FROM bill_tags WHERE user_id = ");
        builder.push_bind(user_id);
        builder.push(" AND tag_id IN (");
        push_postgres_i64_bind_list(&mut builder, tag_ids);
        builder.push("))");
    }
    let row = builder.build().fetch_one(pool).await?;
    let spent_cents: i64 = row.try_get("spent_cents")?;
    Ok(spent_cents)
}
