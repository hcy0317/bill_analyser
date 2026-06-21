#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_postgres_statistics_user_default_currency(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<String> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let currency = sqlx::query(
        "
        SELECT COALESCE(
            NULLIF(metadata->>'defaultCurrency', ''),
            NULLIF(metadata->>'default_currency', ''),
            'CNY'
        ) AS default_currency
        FROM users
        WHERE id = $1
        LIMIT 1
        ",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .and_then(|row| {
        row.try_get::<Option<String>, _>("default_currency")
            .ok()
            .flatten()
    })
    .unwrap_or_else(|| "CNY".to_string());
    let normalized = currency.trim().to_uppercase();
    Ok(if normalized.is_empty() {
        "CNY".to_string()
    } else {
        normalized
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn list_postgres_user_custom_exchange_rates(
    pool: &PostgresPool,
    user_id: UserId,
    base_currency: &str,
) -> DbResult<Vec<UserCustomExchangeRateInput>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let value = load_postgres_custom_exchange_rates_value(pool, user_id, base_currency).await?;
    Ok(parse_postgres_custom_exchange_rates(&value))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn upsert_postgres_user_custom_exchange_rate(
    pool: &PostgresPool,
    user_id: UserId,
    base_currency: &str,
    currency: &str,
    rate: f64,
) -> DbResult<UserCustomExchangeRateUpsert> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let base_currency = base_currency.trim().to_uppercase();
    let currency = currency.trim().to_uppercase();
    let now = chrono::Utc::now();
    let update_time = now.timestamp();
    let effective_date = now.date_naive().to_string();
    let mut rates = parse_postgres_custom_exchange_rates(
        &load_postgres_custom_exchange_rates_value(pool, user_id, &base_currency).await?,
    );
    rates.retain(|item| item.to_currency != currency);
    rates.push(UserCustomExchangeRateInput {
        to_currency: currency,
        rate: decimal_number_text(rate),
        effective_timestamp: Some(update_time),
        effective_date: Some(effective_date),
    });
    rates.sort_by(|left, right| left.to_currency.cmp(&right.to_currency));
    let value = postgres_custom_exchange_rates_value(&rates);
    sqlx::query(
        "
        INSERT INTO settings(user_id, key, value, sensitive)
        VALUES ($1, $2, $3, false)
        ON CONFLICT(user_id, key)
        DO UPDATE SET value = EXCLUDED.value, updated_at = now(), version = settings.version + 1
        ",
    )
    .bind(user_id)
    .bind(postgres_custom_exchange_rates_key(&base_currency))
    .bind(value)
    .execute(pool)
    .await?;
    Ok(UserCustomExchangeRateUpsert { update_time })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn delete_postgres_user_custom_exchange_rate(
    pool: &PostgresPool,
    user_id: UserId,
    base_currency: &str,
    currency: &str,
) -> DbResult<bool> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let base_currency = base_currency.trim().to_uppercase();
    let currency = currency.trim().to_uppercase();
    let mut rates = parse_postgres_custom_exchange_rates(
        &load_postgres_custom_exchange_rates_value(pool, user_id, &base_currency).await?,
    );
    let old_len = rates.len();
    rates.retain(|item| item.to_currency != currency);
    if rates.len() == old_len {
        return Ok(false);
    }
    let value = postgres_custom_exchange_rates_value(&rates);
    sqlx::query(
        "
        INSERT INTO settings(user_id, key, value, sensitive)
        VALUES ($1, $2, $3, false)
        ON CONFLICT(user_id, key)
        DO UPDATE SET value = EXCLUDED.value, updated_at = now(), version = settings.version + 1
        ",
    )
    .bind(user_id)
    .bind(postgres_custom_exchange_rates_key(&base_currency))
    .bind(value)
    .execute(pool)
    .await?;
    Ok(true)
}

async fn load_postgres_custom_exchange_rates_value(
    pool: &PostgresPool,
    user_id: i64,
    base_currency: &str,
) -> DbResult<Value> {
    let key = postgres_custom_exchange_rates_key(base_currency);
    let value = sqlx::query("SELECT value FROM settings WHERE user_id = $1 AND key = $2 LIMIT 1")
        .bind(user_id)
        .bind(key)
        .fetch_optional(pool)
        .await?
        .and_then(|row| row.try_get::<Value, _>("value").ok())
        .unwrap_or_else(|| json!({ "rates": [] }));
    Ok(value)
}

fn postgres_custom_exchange_rates_key(base_currency: &str) -> String {
    format!(
        "statistics.custom_exchange_rates.{}",
        base_currency.trim().to_uppercase()
    )
}

fn parse_postgres_custom_exchange_rates(value: &Value) -> Vec<UserCustomExchangeRateInput> {
    let Some(rates) = value.get("rates").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for item in rates {
        let currency = item
            .get("currency")
            .and_then(Value::as_str)
            .or_else(|| item.get("toCurrency").and_then(Value::as_str))
            .unwrap_or_default()
            .trim()
            .to_uppercase();
        if currency.is_empty() || !seen.insert(currency.clone()) {
            continue;
        }
        let rate = item
            .get("rate")
            .and_then(postgres_value_to_f64)
            .map(decimal_number_text)
            .unwrap_or_else(|| "1.0".to_string());
        let effective_date = item
            .get("effectiveDate")
            .and_then(Value::as_str)
            .or_else(|| item.get("effective_date").and_then(Value::as_str))
            .map(ToOwned::to_owned);
        let effective_timestamp = item
            .get("effectiveTimestamp")
            .and_then(Value::as_i64)
            .or_else(|| item.get("effective_timestamp").and_then(Value::as_i64))
            .or_else(|| effective_date.as_deref().and_then(effective_date_timestamp));
        result.push(UserCustomExchangeRateInput {
            to_currency: currency,
            rate,
            effective_timestamp,
            effective_date,
        });
    }
    result
}

fn postgres_custom_exchange_rates_value(rates: &[UserCustomExchangeRateInput]) -> Value {
    json!({
        "rates": rates
            .iter()
            .map(|item| {
                json!({
                    "currency": item.to_currency.clone(),
                    "rate": item.rate.parse::<f64>().unwrap_or(1.0),
                    "effectiveDate": item.effective_date.clone(),
                    "effectiveTimestamp": item.effective_timestamp
                })
            })
            .collect::<Vec<_>>()
    })
}
