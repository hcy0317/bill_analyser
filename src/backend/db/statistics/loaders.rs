#[tracing::instrument(level = "debug", skip_all)]

async fn load_postgres_statistics_bills(
    pool: &PostgresPool,
    user_id: i64,
    filters: &StatisticsBillFilters,
) -> DbResult<Vec<StatisticsBillInput>> {
    let mut builder = QueryBuilder::<Postgres>::new(
        "SELECT b.id, b.occurred_at, b.transaction_type, b.amount_cents, b.payment_method, b.account_id, b.source_account_id, b.target_account_id, b.transfer_target_account_id, b.standard_payload, ",
    );
    builder.push("COALESCE(NULLIF(b.standard_payload->>'main_category', ''), NULLIF(split_part(c.path, '/', 1), ''), c.name, '') AS main_category, ");
    builder.push("COALESCE(NULLIF(b.standard_payload->>'sub_category', ''), CASE WHEN position('/' in COALESCE(c.path, '')) > 0 THEN substring(c.path from position('/' in c.path) + 1) ELSE '' END, '') AS sub_category, ");
    builder.push("b.merchant, b.description FROM bills b LEFT JOIN categories c ON c.user_id = b.user_id AND c.id = b.category_id WHERE b.user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND b.is_deleted = false");
    if let Some(start_date) = text_filter(filters.start_date.as_deref()) {
        builder.push(" AND b.occurred_at >= ");
        builder.push_bind(start_date);
        builder.push("::date");
    }
    if let Some(end_date) = text_filter(filters.end_date.as_deref()) {
        builder.push(" AND b.occurred_at < (");
        builder.push_bind(end_date);
        builder.push("::date + interval '1 day')");
    }
    if let Some(transaction_type) = text_filter(filters.transaction_type.as_deref()) {
        builder.push(" AND (b.transaction_type = ");
        builder.push_bind(canonical_postgres_transaction_type(&transaction_type));
        builder.push(" OR b.standard_payload->>'type' = ");
        builder.push_bind(transaction_type);
        builder.push(")");
    }
    if let Some(keyword) = text_filter(filters.keyword.as_deref()) {
        let pattern = format!("%{keyword}%");
        builder.push(" AND (b.description ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR b.merchant ILIKE ");
        builder.push_bind(pattern);
        builder.push(")");
    }
    builder.push(" ORDER BY b.occurred_at ASC, b.id ASC");

    let rows = builder.build().fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            let payload: Value = row.try_get("standard_payload")?;
            let bill_type = row
                .try_get::<Option<String>, _>("transaction_type")?
                .unwrap_or_default();
            let amount_cents: i64 = row.try_get("amount_cents")?;
            let amount_cents = signed_postgres_statistics_amount_cents(&bill_type, amount_cents);
            Ok(StatisticsBillInput {
                id: row.try_get("id")?,
                date: postgres_timestamp_text(row.try_get("occurred_at")?),
                bill_type,
                amount_cents,
                channel: row
                    .try_get::<Option<String>, _>("payment_method")?
                    .unwrap_or_default(),
                source_account_id: positive_i64(
                    row.try_get::<Option<i64>, _>("source_account_id")?
                        .or(row.try_get::<Option<i64>, _>("account_id")?),
                ),
                destination_account_id: positive_i64(
                    row.try_get::<Option<i64>, _>("target_account_id")?
                        .or(row.try_get::<Option<i64>, _>("transfer_target_account_id")?),
                ),
                destination_account: String::new(),
                destination_amount_cents: payload
                    .get("destination_amount_cents")
                    .or_else(|| payload.get("destinationAmountCents"))
                    .and_then(postgres_value_to_i64),
                main_category: row
                    .try_get::<Option<String>, _>("main_category")?
                    .unwrap_or_default(),
                sub_category: row
                    .try_get::<Option<String>, _>("sub_category")?
                    .unwrap_or_default(),
                counterparty: row
                    .try_get::<Option<String>, _>("merchant")?
                    .unwrap_or_default(),
                description: row
                    .try_get::<Option<String>, _>("description")?
                    .unwrap_or_default(),
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_postgres_statistics_categories(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<StatisticsCategoryInput>> {
    let rows =
        sqlx::query("SELECT id, path, name FROM categories WHERE user_id = $1 ORDER BY id ASC")
            .bind(user_id)
            .fetch_all(pool)
            .await?;
    rows.into_iter()
        .map(|row| {
            let path: Option<String> = row.try_get("path")?;
            let name: String = row.try_get("name")?;
            let (main_category, sub_category) =
                postgres_category_names_from_path(path.as_deref(), &name);
            Ok(StatisticsCategoryInput {
                id: row.try_get("id")?,
                main_category,
                sub_category,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_postgres_statistics_accounts(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<StatisticsAccountInput>> {
    let rows = sqlx::query(
        r#"
        SELECT id, name, account_type, is_active, balance_cents, currency, metadata
        FROM accounts
        WHERE user_id = $1
        ORDER BY id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let metadata: Value = row.try_get("metadata")?;
            let balance_cents: i64 = row.try_get("balance_cents")?;
            let initial_balance_cents = postgres_initial_balance_cents(&metadata, balance_cents);
            Ok(StatisticsAccountInput {
                id: row.try_get("id")?,
                name: row
                    .try_get::<Option<String>, _>("name")?
                    .unwrap_or_default(),
                account_type: row
                    .try_get::<Option<String>, _>("account_type")?
                    .unwrap_or_default(),
                hidden: !row.try_get::<bool, _>("is_active")?,
                balance_cents,
                initial_balance_cents,
                currency: row.try_get("currency")?,
                icon: metadata
                    .get("icon")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_postgres_calendar_recurring_rules(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<RecurringRuleInput>> {
    let rows = sqlx::query(
        r#"
        SELECT
            id,
            name,
            source_amount_minor_units,
            transaction_type,
            scheduled_frequency,
            scheduled_next_date
        FROM transaction_templates
        WHERE user_id = $1
          AND template_type = 2
          AND enabled = true
          AND hidden = false
        ORDER BY display_order ASC, name ASC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let amount_minor: i64 = row.try_get("source_amount_minor_units")?;
            Ok(RecurringRuleInput {
                id: Some(row.try_get("id")?),
                name: row
                    .try_get::<Option<String>, _>("name")?
                    .unwrap_or_default(),
                amount_cents: amount_minor,
                bill_type: row
                    .try_get::<Option<String>, _>("transaction_type")?
                    .unwrap_or_default(),
                frequency: row
                    .try_get::<Option<String>, _>("scheduled_frequency")?
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "monthly".to_string()),
                next_date: row
                    .try_get::<Option<String>, _>("scheduled_next_date")?
                    .unwrap_or_default(),
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_postgres_account_balance_deltas_before(
    pool: &PostgresPool,
    user_id: i64,
    start_date: NaiveDate,
) -> DbResult<BTreeMap<i64, i64>> {
    let filters = StatisticsBillFilters {
        end_date: Some((start_date - chrono::Duration::days(1)).to_string()),
        ..StatisticsBillFilters::default()
    };
    let bills = load_postgres_statistics_bills(pool, user_id, &filters).await?;
    let account_ids = load_postgres_statistics_accounts(pool, user_id)
        .await?
        .into_iter()
        .map(|account| account.id)
        .collect::<Vec<_>>();
    let mut deltas = BTreeMap::new();
    for account_id in account_ids {
        let mut cents = 0_i64;
        for bill in &bills {
            let amount_cents = bill.amount_cents.abs();
            if is_income_type(&bill.bill_type) && bill.source_account_id == Some(account_id) {
                cents += amount_cents;
            } else if is_expense_type(&bill.bill_type) && bill.source_account_id == Some(account_id)
            {
                cents -= amount_cents;
            } else if is_transfer_type(&bill.bill_type) {
                if bill.source_account_id == Some(account_id) {
                    cents -= amount_cents;
                }
                if bill.destination_account_id == Some(account_id) {
                    let destination_cents = bill
                        .destination_amount_cents
                        .map(i64::abs)
                        .filter(|value| *value != 0)
                        .unwrap_or(amount_cents);
                    cents += destination_cents;
                }
            }
        }
        deltas.insert(account_id, cents);
    }
    Ok(deltas)
}
