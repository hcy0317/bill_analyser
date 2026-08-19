async fn query_postgres_budgets_raw(
    pool: &PostgresPool,
    user_id: i64,
    filters: &BudgetFilters,
) -> DbResult<Vec<BudgetRecord>> {
    let period_type = text_filter(filters.period_type.as_deref());
    let category = text_filter(filters.category.as_deref());
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, name, category, sub_category, period_type,
            amount_cents, start_date, end_date, alert_threshold, enabled,
            created_at, updated_at
        FROM budgets
        WHERE user_id = $1
          AND ($2::TEXT IS NULL OR period_type = $2)
          AND ($3::BOOLEAN IS NULL OR enabled = $3)
          AND ($4::TEXT IS NULL OR category = $4)
        ORDER BY created_at DESC, id DESC
        "#,
    )
    .bind(user_id)
    .bind(period_type)
    .bind(filters.enabled)
    .bind(category)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(budget_record_from_postgres_row)
        .collect::<DbResult<Vec<_>>>()
}

async fn enrich_postgres_budget_listing(
    pool: &PostgresPool,
    user_id: i64,
    budget_type: Option<i32>,
    budgets: &mut [BudgetRecord],
) -> DbResult<Vec<BudgetRecord>> {
    let categories = load_postgres_category_context_values(pool, user_id).await?;
    let category_context = build_budget_category_context(&categories);
    let mut enriched = Vec::new();
    for budget in budgets {
        let resolved_type = resolve_budget_category_type(
            &category_context,
            &record_text(budget, "category"),
            Some(&normalize_sub_category(record_value(
                budget,
                "sub_category",
            ))),
            budget_type,
        )
        .map(|value| value.code());
        if budget_type.is_some() && resolved_type != budget_type {
            continue;
        }
        if let Some(resolved_type) = resolved_type {
            let category_info = resolve_budget_category_info(
                &category_context,
                &record_text(budget, "category"),
                Some(&normalize_sub_category(record_value(
                    budget,
                    "sub_category",
                ))),
                Some(resolved_type),
            );
            budget.insert("type".to_string(), json_i64(i64::from(resolved_type)));
            budget.insert(
                "category_id".to_string(),
                category_info
                    .get("id")
                    .and_then(value_to_i64)
                    .map(|id| Value::String(id.to_string()))
                    .unwrap_or_else(|| Value::String(String::new())),
            );
            budget.insert("category_info".to_string(), category_info);
        } else {
            budget.insert("category_info".to_string(), Value::Null);
            budget.insert("category_id".to_string(), Value::String(String::new()));
        }
        enriched.push(budget.clone());
    }
    Ok(enriched)
}

async fn load_postgres_category_context_values(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<Value>> {
    let rows = sqlx::query(
        r#"
        SELECT id, category_type, path, name, icon, color
        FROM categories
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let path: Option<String> = row.try_get("path")?;
            let name: String = row.try_get("name")?;
            let (main_category, sub_category) =
                category_names_from_postgres_path(path.as_deref(), &name);
            Ok(json!({
                "id": row.try_get::<i64, _>("id")?,
                "type": row
                    .try_get::<Option<String>, _>("category_type")?
                    .and_then(|value| value.trim().parse::<i32>().ok()),
                "main_category": main_category,
                "sub_category": sub_category,
                "icon": row.try_get::<Option<String>, _>("icon")?.unwrap_or_default(),
                "color": row.try_get::<Option<String>, _>("color")?.unwrap_or_default(),
            }))
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(DbError::from)
}

async fn load_postgres_category_filter(
    pool: &PostgresPool,
    user_id: i64,
    category_id: i64,
) -> DbResult<Option<(String, String)>> {
    sqlx::query(
        r#"
        SELECT path, name
        FROM categories
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(category_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .map(|row| {
        let path: Option<String> = row.try_get("path")?;
        let name: String = row.try_get("name")?;
        Ok(category_names_from_postgres_path(path.as_deref(), &name))
    })
    .transpose()
}
