fn push_bill_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    user_id: i64,
    filters: &BillFilters,
) {
    if let Some(id) = filters.id.filter(|value| *value > 0) {
        builder.push(" AND b.id = ");
        builder.push_bind(id);
    }
    if let Some(value) = text_filter(filters.date_from.as_deref()) {
        builder.push(" AND b.occurred_at >= ");
        builder.push_bind(value);
        builder.push("::date");
    }
    if let Some(value) = text_filter(filters.date_to.as_deref()) {
        builder.push(" AND b.occurred_at < (");
        builder.push_bind(value);
        builder.push("::date + interval '1 day')");
    }
    if let Some(value) = text_filter(filters.date_before.as_deref()) {
        builder.push(" AND b.occurred_at < ");
        builder.push_bind(value);
        builder.push("::date");
    }
    if let Some(value) = text_filter(filters.transaction_type.as_deref()) {
        let canonical = canonical_transaction_type(&value);
        builder.push(" AND (b.transaction_type = ");
        builder.push_bind(canonical.clone());
        builder.push(" OR b.standard_payload->>'type' = ");
        builder.push_bind(value);
        builder.push(")");
    }
    if let Some(value) = text_filter(filters.main_category.as_deref()) {
        builder.push(" AND ");
        push_postgres_bill_main_category_expr(builder);
        builder.push(" = ");
        builder.push_bind(value);
    }
    if let Some(value) = text_filter(filters.sub_category.as_deref()) {
        builder.push(" AND ");
        push_postgres_bill_sub_category_expr(builder);
        builder.push(" = ");
        builder.push_bind(value);
    }
    if let Some(value) = text_filter(filters.batch_id.as_deref()) {
        builder.push(" AND b.standard_payload->>'batch_id' = ");
        builder.push_bind(value);
    }
    if let Some(value) = text_filter(filters.counterparty.as_deref()) {
        builder.push(" AND b.merchant ILIKE ");
        builder.push_bind(format!("%{value}%"));
    }
    if let Some(value) = text_filter(filters.description.as_deref()) {
        builder.push(" AND b.description ILIKE ");
        builder.push_bind(format!("%{value}%"));
    }
    if let Some(value) = text_filter(filters.keyword.as_deref()) {
        let pattern = format!("%{value}%");
        builder.push(" AND (b.description ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR b.merchant ILIKE ");
        builder.push_bind(pattern);
        builder.push(")");
    }
    let account_ids = normalize_ids(&filters.account_ids);
    if !account_ids.is_empty() {
        if let Some(flow_direction @ ("inflow" | "outflow")) = filters.flow_direction.as_deref() {
            push_account_flow_filter(builder, &account_ids, flow_direction);
        } else {
            builder.push(" AND (b.account_id IN (");
            push_bind_list(builder, &account_ids);
            builder.push(") OR b.source_account_id IN (");
            push_bind_list(builder, &account_ids);
            builder.push(") OR b.target_account_id IN (");
            push_bind_list(builder, &account_ids);
            builder.push(") OR b.transfer_target_account_id IN (");
            push_bind_list(builder, &account_ids);
            builder.push("))");
        }
    }
    push_category_filters(builder, filters);
    let tag_ids = normalize_ids(&filters.tag_ids);
    if !tag_ids.is_empty() {
        builder.push(" AND b.id IN (SELECT bill_id FROM bill_tags WHERE user_id = ");
        builder.push_bind(user_id);
        builder.push(" AND tag_id IN (");
        push_bind_list(builder, &tag_ids);
        builder.push("))");
    }
    if let Some(value) = filters.min_amount_cents {
        builder.push(" AND ABS(b.amount_cents) >= ");
        builder.push_bind(value);
    }
    if let Some(value) = filters.max_amount_cents {
        builder.push(" AND ABS(b.amount_cents) <= ");
        builder.push_bind(value);
    }
    if let Some(value) = text_filter(filters.amount_filter_cents.as_deref()) {
        push_amount_filter_cents(builder, &value);
    }
}

fn push_account_flow_filter(
    builder: &mut QueryBuilder<'_, Postgres>,
    account_ids: &[i64],
    flow_direction: &str,
) {
    builder.push(" AND ((b.transaction_type = ");
    builder.push_bind(if flow_direction == "inflow" { "income" } else { "expense" });
    builder.push(" AND (b.account_id IN (");
    push_bind_list(builder, account_ids);
    builder.push(") OR b.source_account_id IN (");
    push_bind_list(builder, account_ids);
    builder.push("))) OR (b.transaction_type IN ('transfer', 'investment') AND ");
    if flow_direction == "inflow" {
        builder.push("(b.target_account_id IN (");
        push_bind_list(builder, account_ids);
        builder.push(") OR b.transfer_target_account_id IN (");
        push_bind_list(builder, account_ids);
        builder.push("))");
    } else {
        builder.push("(b.source_account_id IN (");
        push_bind_list(builder, account_ids);
        builder.push(") OR b.account_id IN (");
        push_bind_list(builder, account_ids);
        builder.push("))");
    }
    builder.push("))");
}

fn push_category_filters(builder: &mut QueryBuilder<'_, Postgres>, filters: &BillFilters) {
    let categories = filters
        .categories
        .iter()
        .filter_map(|category| {
            text_filter(Some(&category.main)).map(|main| {
                (
                    main,
                    category
                        .sub
                        .as_deref()
                        .and_then(|value| text_filter(Some(value))),
                )
            })
        })
        .collect::<Vec<_>>();
    if categories.is_empty() {
        return;
    }
    builder.push(" AND (");
    for (index, (main, sub)) in categories.into_iter().enumerate() {
        if index > 0 {
            builder.push(" OR ");
        }
        builder.push("(");
        push_postgres_bill_main_category_expr(builder);
        builder.push(" = ");
        builder.push_bind(main);
        if let Some(sub) = sub {
            builder.push(" AND ");
            push_postgres_bill_sub_category_expr(builder);
            builder.push(" = ");
            builder.push_bind(sub);
        }
        builder.push(")");
    }
    builder.push(")");
}

fn push_amount_filter_cents(builder: &mut QueryBuilder<'_, Postgres>, amount_filter_cents: &str) {
    let parts = amount_filter_cents.split(':').collect::<Vec<_>>();
    if parts.len() < 2 {
        return;
    }
    let amount_cents = |index: usize| -> Option<i64> {
        parts.get(index)?.trim().parse::<i64>().ok()?.checked_abs()
    };
    match parts[0].to_ascii_lowercase().as_str() {
        "eq" => push_amount_condition(builder, " = ", amount_cents(1)),
        "ne" => push_amount_condition(builder, " != ", amount_cents(1)),
        "gt" => push_amount_condition(builder, " > ", amount_cents(1)),
        "lt" => push_amount_condition(builder, " < ", amount_cents(1)),
        "gte" => push_amount_condition(builder, " >= ", amount_cents(1)),
        "lte" => push_amount_condition(builder, " <= ", amount_cents(1)),
        "between" => {
            if let (Some(minimum), Some(maximum)) = (amount_cents(1), amount_cents(2)) {
                builder.push(" AND ABS(b.amount_cents) BETWEEN ");
                builder.push_bind(minimum);
                builder.push(" AND ");
                builder.push_bind(maximum);
            }
        }
        _ => {}
    }
}

fn push_amount_condition(
    builder: &mut QueryBuilder<'_, Postgres>,
    operator: &'static str,
    amount: Option<i64>,
) {
    if let Some(amount) = amount {
        builder.push(" AND ABS(b.amount_cents)");
        builder.push(operator);
        builder.push_bind(amount);
    }
}

fn push_bind_list(builder: &mut QueryBuilder<'_, Postgres>, values: &[i64]) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            builder.push(", ");
        }
        builder.push_bind(*value);
    }
}
