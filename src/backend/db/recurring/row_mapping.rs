fn recurring_suggestion_select_sql(where_clause: &str, page_clause: &str) -> String {
    format!(
        r#"
        SELECT id, user_id, pattern_hash, name, description, type, amount_cents,
               source_account_id, destination_account_id, counterparty, frequency,
               detected_interval_days::DOUBLE PRECISION AS detected_interval_days,
               confidence_score::DOUBLE PRECISION AS confidence_score,
               sample_count,
               sample_bill_ids, first_occurrence, last_occurrence,
               suggested_next_date, status, created_at, updated_at
        FROM recurring_suggestions
        {where_clause}
        ORDER BY confidence_score DESC, last_occurrence DESC, id DESC
        {page_clause}
        "#
    )
}

fn postgres_recurring_suggestion_from_row(row: &PgRow) -> DbResult<Value> {
    let amount_cents: i64 = row.try_get("amount_cents")?;
    let sample_bill_ids: Value = row.try_get("sample_bill_ids")?;
    Ok(json!({
        "id": row.try_get::<i64, _>("id")?,
        "user_id": row.try_get::<i64, _>("user_id")?,
        "pattern_hash": row.try_get::<String, _>("pattern_hash")?,
        "name": row.try_get::<String, _>("name")?,
        "description": row.try_get::<Option<String>, _>("description")?,
        "type": row.try_get::<String, _>("type")?,
        "amount_cents": amount_cents,
        "source_account_id": row.try_get::<Option<i64>, _>("source_account_id")?,
        "destination_account_id": row.try_get::<Option<i64>, _>("destination_account_id")?,
        "counterparty": row.try_get::<Option<String>, _>("counterparty")?,
        "frequency": row.try_get::<String, _>("frequency")?,
        "detected_interval_days": row.try_get::<Option<f64>, _>("detected_interval_days")?,
        "confidence_score": row.try_get::<f64, _>("confidence_score")?,
        "sample_count": row.try_get::<i32, _>("sample_count")?,
        "sample_bill_ids_json": sample_bill_ids.to_string(),
        "sample_bill_ids": sample_bill_ids,
        "first_occurrence": row.try_get::<Option<String>, _>("first_occurrence")?,
        "last_occurrence": row.try_get::<Option<String>, _>("last_occurrence")?,
        "suggested_next_date": row.try_get::<Option<String>, _>("suggested_next_date")?,
        "status": row.try_get::<String, _>("status")?,
        "created_at": row.try_get::<chrono::DateTime<Utc>, _>("created_at")?.to_rfc3339_opts(SecondsFormat::Secs, true),
        "updated_at": row.try_get::<chrono::DateTime<Utc>, _>("updated_at")?.to_rfc3339_opts(SecondsFormat::Secs, true),
    }))
}

fn postgres_recent_bill_from_row(row: &PgRow) -> DbResult<Value> {
    let payload = row.try_get::<Value, _>("standard_payload")?;
    let category_path = row.try_get::<Option<String>, _>("category_path")?;
    let category_name = row.try_get::<Option<String>, _>("category_name")?;
    let (main_category, sub_category) = category_names_from_postgres_values(
        &payload,
        category_path.as_deref(),
        category_name.as_deref(),
    );
    let amount_cents: i64 = row.try_get("amount_cents")?;
    let destination_account_id =
        row.try_get::<Option<i64>, _>("target_account_id")?
            .or(row.try_get::<Option<i64>, _>("transfer_target_account_id")?);
    Ok(json!({
        "id": row.try_get::<i64, _>("id")?,
        "date": row.try_get::<chrono::DateTime<Utc>, _>("occurred_at")?.to_rfc3339_opts(SecondsFormat::Secs, true),
        "type": row.try_get::<Option<String>, _>("transaction_type")?.unwrap_or_default(),
        "amount_cents": amount_cents,
        "counterparty": row.try_get::<Option<String>, _>("merchant")?.unwrap_or_default(),
        "description": row.try_get::<Option<String>, _>("description")?.unwrap_or_default(),
        "main_category": main_category,
        "sub_category": sub_category,
        "source_account_id": row.try_get::<Option<i64>, _>("source_account_id")?,
        "destination_account_id": destination_account_id,
    }))
}

fn category_names_from_postgres_values(
    payload: &Value,
    category_path: Option<&str>,
    category_name: Option<&str>,
) -> (String, String) {
    let payload_main = payload
        .get("main_category")
        .or_else(|| payload.get("mainCategory"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let payload_sub = payload
        .get("sub_category")
        .or_else(|| payload.get("subCategory"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if payload_main.is_some() || payload_sub.is_some() {
        return (
            payload_main.unwrap_or_default(),
            payload_sub.unwrap_or_default(),
        );
    }
    let parts = category_path
        .unwrap_or_default()
        .split('/')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    match parts.as_slice() {
        [] => (category_name.unwrap_or_default().to_string(), String::new()),
        [main] => ((*main).to_string(), String::new()),
        [main, rest @ ..] => ((*main).to_string(), rest.join("/")),
    }
}
