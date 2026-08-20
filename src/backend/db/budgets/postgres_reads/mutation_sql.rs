async fn get_postgres_budget_by_id_on_pool(
    pool: &PostgresPool,
    user_id: i64,
    budget_id: i64,
) -> DbResult<Option<BudgetRecord>> {
    sqlx::query(
        r#"
        SELECT id, user_id, name, category, sub_category, period_type,
            amount_cents, start_date, end_date, alert_threshold, enabled,
            created_at, updated_at
        FROM budgets
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(budget_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .map(budget_record_from_postgres_row)
    .transpose()
}

async fn get_postgres_budget_by_id_on_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i64,
    budget_id: i64,
) -> DbResult<Option<BudgetRecord>> {
    sqlx::query(
        r#"
        SELECT id, user_id, name, category, sub_category, period_type,
            amount_cents, start_date, end_date, alert_threshold, enabled,
            created_at, updated_at
        FROM budgets
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(budget_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .map(budget_record_from_postgres_row)
    .transpose()
}

async fn insert_postgres_budget_on_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i64,
    payload: &BudgetRecord,
) -> DbResult<i64> {
    let row = sqlx::query(
        r#"
        INSERT INTO budgets (
            user_id, name, category, sub_category, period_type, amount_cents,
            start_date, end_date, alert_threshold, enabled, created_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, now(), now())
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(value_string(payload.get("name")))
    .bind(optional_text(payload.get("category")))
    .bind(normalize_sub_category(payload.get("sub_category")))
    .bind(required_text(payload, "period_type")?)
    .bind(amount_cents_from_record(payload, "amount_cents")?)
    .bind(required_date(payload, "start_date")?)
    .bind(optional_date(payload.get("end_date"))?)
    .bind(i32_value(payload.get("alert_threshold")).unwrap_or(80))
    .bind(bool_value(payload.get("enabled")).unwrap_or(true))
    .fetch_one(&mut **tx)
    .await?;
    row.try_get("id").map_err(DbError::from)
}

async fn update_postgres_budget_on_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i64,
    budget_id: i64,
    payload: &BudgetRecord,
) -> DbResult<bool> {
    let mut keys = payload
        .keys()
        .filter(|key| BUDGET_UPDATE_COLUMNS.contains(&key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    if keys.is_empty() {
        return Err(DbError::InvalidOperation("empty budget update".to_string()));
    }

    let mut builder = QueryBuilder::<Postgres>::new("UPDATE budgets SET ");
    let mut first_assignment = true;
    for key in keys {
        if !first_assignment {
            builder.push(", ");
        }
        first_assignment = false;
        let value = payload.get(&key);
        match key.as_str() {
            "name" => {
                builder.push("name = ").push_bind(value_string(value));
            }
            "category" => {
                builder.push("category = ").push_bind(optional_text(value));
            }
            "sub_category" => {
                builder
                    .push("sub_category = ")
                    .push_bind(normalize_sub_category(value));
            }
            "period_type" => {
                builder
                    .push("period_type = ")
                    .push_bind(required_text(payload, "period_type")?);
            }
            "amount_cents" => {
                builder
                    .push("amount_cents = ")
                    .push_bind(amount_cents_from_record(payload, "amount_cents")?);
            }
            "start_date" => {
                builder
                    .push("start_date = ")
                    .push_bind(required_date(payload, "start_date")?);
            }
            "end_date" => {
                builder.push("end_date = ").push_bind(optional_date(value)?);
            }
            "alert_threshold" => {
                builder
                    .push("alert_threshold = ")
                    .push_bind(i32_value(value).unwrap_or(80));
            }
            "enabled" => {
                builder
                    .push("enabled = ")
                    .push_bind(bool_value(value).unwrap_or(true));
            }
            "updated_at" => {
                builder.push("updated_at = now()");
            }
            invalid => {
                return Err(DbError::InvalidOperation(format!(
                    "invalid budget update field: {invalid}"
                )));
            }
        }
    }
    builder
        .push(" WHERE id = ")
        .push_bind(budget_id)
        .push(" AND user_id = ")
        .push_bind(user_id);
    let result = builder.build().execute(&mut **tx).await?;
    Ok(result.rows_affected() > 0)
}

async fn import_postgres_budget_row_on_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i64,
    budget: &BudgetRecord,
) -> DbResult<ImportBudgetRowAction> {
    let name = record_text(budget, "name");
    let existing_id =
        sqlx::query("SELECT id FROM budgets WHERE name = $1 AND user_id = $2 ORDER BY id LIMIT 1")
            .bind(&name)
            .bind(user_id)
            .fetch_optional(&mut **tx)
            .await?
            .map(|row| row.try_get::<i64, _>("id"))
            .transpose()?;
    if let Some(existing_id) = existing_id {
        let mut update_payload = budget.clone();
        update_payload.remove("name");
        update_payload.insert("updated_at".to_string(), Value::Null);
        update_postgres_budget_on_tx(tx, user_id, existing_id, &update_payload).await?;
        Ok(ImportBudgetRowAction::Updated)
    } else {
        let mut payload = budget.clone();
        payload.insert("name".to_string(), Value::String(name));
        insert_postgres_budget_on_tx(tx, user_id, &payload).await?;
        Ok(ImportBudgetRowAction::Created)
    }
}
