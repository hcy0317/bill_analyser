async fn get_postgres_recurring_template_name(
    pool: &PostgresPool,
    user_id: i64,
    recurring_id: i64,
) -> DbResult<Option<String>> {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT name FROM transaction_templates WHERE user_id = $1 AND id = $2 AND template_type = 2",
    )
    .bind(user_id)
    .bind(recurring_id)
    .fetch_optional(pool)
    .await
    .map(|value| value.flatten())
    .map_err(DbError::from)
}

async fn list_enabled_postgres_recurring_templates(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<BillRecord>> {
    let rows = sqlx::query(&postgres_recurring_template_select_sql(
        "WHERE user_id = $1 AND template_type = 2 AND enabled = true",
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(postgres_recurring_template_from_row)
        .collect()
}

async fn get_postgres_recurring_template_on_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    recurring_id: i64,
) -> DbResult<Option<BillRecord>> {
    let row = sqlx::query(&postgres_recurring_template_select_sql(
        "WHERE user_id = $1 AND id = $2 AND template_type = 2",
    ))
    .bind(user_id)
    .bind(recurring_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.as_ref()
        .map(postgres_recurring_template_from_row)
        .transpose()
}

fn postgres_recurring_template_select_sql(where_clause: &str) -> String {
    format!(
        r#"
        SELECT id, user_id, id AS template_id, name, description, transaction_type,
               category_id, source_account_id, destination_account_id,
               source_amount_minor_units, destination_amount_minor_units,
               hide_amount, tag_ids, comment, scheduled_frequency,
               scheduled_frequency_type, scheduled_start_date, scheduled_end_date,
               scheduled_next_date, enabled, auto_create, display_order, hidden,
               utc_offset, created_at, updated_at
        FROM transaction_templates
        {where_clause}
        ORDER BY display_order ASC, name ASC, id ASC
        "#
    )
}

fn postgres_recurring_template_from_row(row: &PgRow) -> DbResult<BillRecord> {
    let tag_ids: Value = row.try_get("tag_ids")?;
    let tag_text = tag_ids
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(pg_value_string)
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    let mut record = Map::new();
    record.insert("id".to_string(), json!(row.try_get::<i64, _>("id")?));
    record.insert(
        "user_id".to_string(),
        json!(row.try_get::<i64, _>("user_id")?),
    );
    record.insert(
        "template_id".to_string(),
        optional_i64_json(row.try_get::<Option<i64>, _>("template_id")?),
    );
    record.insert(
        "name".to_string(),
        pg_optional_string_json(row.try_get("name")?),
    );
    record.insert(
        "description".to_string(),
        pg_optional_string_json(row.try_get("description")?),
    );
    record.insert(
        "type".to_string(),
        pg_optional_string_json(row.try_get("transaction_type")?),
    );
    record.insert(
        "category".to_string(),
        pg_optional_string_json(row.try_get("category_id")?),
    );
    record.insert(
        "account".to_string(),
        pg_optional_string_json(row.try_get("source_account_id")?),
    );
    record.insert(
        "counterparty".to_string(),
        pg_optional_string_json(row.try_get("destination_account_id")?),
    );
    record.insert(
        "source_amount_cents".to_string(),
        json!(row.try_get::<i64, _>("source_amount_minor_units")?),
    );
    record.insert(
        "destination_amount_cents".to_string(),
        json!(row.try_get::<i64, _>("destination_amount_minor_units")?),
    );
    record.insert(
        "hide_amount".to_string(),
        json!(i64::from(row.try_get::<bool, _>("hide_amount")?)),
    );
    record.insert("tag".to_string(), Value::String(tag_text));
    record.insert(
        "comment".to_string(),
        pg_optional_string_json(row.try_get("comment")?),
    );
    record.insert(
        "frequency".to_string(),
        pg_optional_string_json(row.try_get("scheduled_frequency")?),
    );
    record.insert(
        "scheduled_frequency_type".to_string(),
        optional_i64_json(
            row.try_get::<Option<i32>, _>("scheduled_frequency_type")?
                .map(i64::from),
        ),
    );
    record.insert(
        "start_date".to_string(),
        pg_optional_string_json(row.try_get("scheduled_start_date")?),
    );
    record.insert(
        "end_date".to_string(),
        pg_optional_string_json(row.try_get("scheduled_end_date")?),
    );
    record.insert(
        "next_date".to_string(),
        pg_optional_string_json(row.try_get("scheduled_next_date")?),
    );
    record.insert(
        "enabled".to_string(),
        json!(i64::from(row.try_get::<bool, _>("enabled")?)),
    );
    record.insert(
        "auto_create".to_string(),
        json!(i64::from(row.try_get::<bool, _>("auto_create")?)),
    );
    record.insert(
        "display_order".to_string(),
        json!(i64::from(row.try_get::<i32, _>("display_order")?)),
    );
    record.insert(
        "hidden".to_string(),
        json!(i64::from(row.try_get::<bool, _>("hidden")?)),
    );
    record.insert(
        "utc_offset".to_string(),
        json!(i64::from(row.try_get::<i32, _>("utc_offset")?)),
    );
    record.insert(
        "created_at".to_string(),
        Value::String(
            row.try_get::<chrono::DateTime<Utc>, _>("created_at")?
                .to_rfc3339_opts(SecondsFormat::Secs, true),
        ),
    );
    record.insert(
        "updated_at".to_string(),
        Value::String(
            row.try_get::<chrono::DateTime<Utc>, _>("updated_at")?
                .to_rfc3339_opts(SecondsFormat::Secs, true),
        ),
    );
    Ok(record)
}

fn build_postgres_recurring_candidates_for_bill_data(
    bill: &BillRecord,
    recurring_rows: &[BillRecord],
    linked_recurring_id: Option<i64>,
    tolerance_days: i64,
) -> Vec<Value> {
    let Some(bill_date) = pg_parse_date_value(&pg_record_text(bill, "date")) else {
        return Vec::new();
    };
    let bill_type = pg_normalize_template_transaction_type(bill.get("type"));
    let bill_amount_cents = pg_record_optional_i64(bill, "amount_cents")
        .unwrap_or_default()
        .abs();
    let bill_source_account = pg_record_text(bill, "source_account_id");
    let bill_destination_account = pg_record_text(bill, "destination_account_id");
    let mut candidates = Vec::new();

    for recurring in recurring_rows {
        if pg_normalize_template_transaction_type(recurring.get("type")) != bill_type {
            continue;
        }
        let recurring_amount_cents = pg_recurring_i64(recurring, "source_amount_cents")
            .unwrap_or(0)
            .abs();
        if recurring_amount_cents != bill_amount_cents {
            continue;
        }
        let Some(matched_occurrence) =
            pg_find_recurring_occurrence_near_date(recurring, bill_date, tolerance_days)
        else {
            continue;
        };

        let mut score = 80_i64;
        let mut reasons = vec![
            Value::String("type".to_string()),
            Value::String("amount".to_string()),
            Value::String("schedule".to_string()),
        ];
        if pg_recurring_text(recurring, "account") == bill_source_account {
            reasons.push(Value::String("source_account".to_string()));
            score += 10;
        }
        if !matches!(bill_destination_account.as_str(), "" | "0")
            && pg_recurring_text(recurring, "counterparty") == bill_destination_account
        {
            reasons.push(Value::String("destination_account".to_string()));
            score += 10;
        }
        let days_offset = (matched_occurrence - bill_date).num_days().abs();
        score += (10 - days_offset * 2).max(0);

        let mut candidate = serialize_postgres_recurring_template_row(recurring);
        candidate.insert("matchScore".to_string(), json!(score));
        candidate.insert("matchReasons".to_string(), Value::Array(reasons));
        candidate.insert(
            "matchedOccurrenceDate".to_string(),
            Value::String(matched_occurrence.to_string()),
        );
        candidate.insert("matchedDayOffset".to_string(), json!(days_offset));
        candidate.insert(
            "linked".to_string(),
            Value::Bool(
                linked_recurring_id
                    .zip(pg_recurring_i64(recurring, "id"))
                    .is_some_and(|(linked, recurring_id)| linked == recurring_id),
            ),
        );
        candidates.push(Value::Object(candidate));
    }

    candidates.sort_by(|left, right| {
        let left = left.as_object().expect("candidate object");
        let right = right.as_object().expect("candidate object");
        pg_recurring_i64(right, "matchScore")
            .cmp(&pg_recurring_i64(left, "matchScore"))
            .then_with(|| {
                pg_recurring_i64(left, "matchedDayOffset")
                    .unwrap_or(999)
                    .cmp(&pg_recurring_i64(right, "matchedDayOffset").unwrap_or(999))
            })
            .then_with(|| pg_recurring_text(left, "name").cmp(&pg_recurring_text(right, "name")))
    });
    candidates
}

fn serialize_postgres_recurring_template_row(row: &BillRecord) -> Map<String, Value> {
    let mut value = Map::new();
    value.insert("id".to_string(), pg_recurring_text(row, "id").into());
    value.insert("timeSequenceId".to_string(), String::new().into());
    value.insert("templateType".to_string(), json!(2));
    value.insert("name".to_string(), pg_recurring_text(row, "name").into());
    value.insert(
        "type".to_string(),
        json!(pg_normalize_template_transaction_type(row.get("type"))),
    );
    value.insert(
        "categoryId".to_string(),
        pg_recurring_text(row, "category").into(),
    );
    value.insert("time".to_string(), json!(0));
    value.insert(
        "utcOffset".to_string(),
        json!(pg_recurring_i64(row, "utc_offset").unwrap_or(0)),
    );
    value.insert(
        "sourceAccountId".to_string(),
        pg_recurring_text(row, "account").into(),
    );
    value.insert(
        "destinationAccountId".to_string(),
        pg_recurring_text(row, "counterparty").into(),
    );
    value.insert(
        "sourceAmountCents".to_string(),
        json!(pg_recurring_i64(row, "source_amount_cents").unwrap_or(0)),
    );
    value.insert(
        "destinationAmountCents".to_string(),
        json!(pg_recurring_i64(row, "destination_amount_cents").unwrap_or(0)),
    );
    value.insert(
        "hideAmount".to_string(),
        Value::Bool(pg_recurring_i64(row, "hide_amount").unwrap_or(0) != 0),
    );
    value.insert(
        "tagIds".to_string(),
        Value::Array(
            pg_recurring_text(row, "tag")
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(|item| Value::String(item.to_string()))
                .collect(),
        ),
    );
    value.insert(
        "comment".to_string(),
        pg_recurring_text(row, "comment").into(),
    );
    value.insert("editable".to_string(), Value::Bool(true));
    value.insert(
        "displayOrder".to_string(),
        json!(pg_recurring_i64(row, "display_order").unwrap_or(0)),
    );
    value.insert(
        "hidden".to_string(),
        Value::Bool(pg_recurring_i64(row, "hidden").unwrap_or(0) != 0),
    );
    value.insert(
        "scheduledFrequencyType".to_string(),
        json!(pg_recurring_i64(row, "scheduled_frequency_type").unwrap_or(0)),
    );
    value.insert(
        "scheduledFrequency".to_string(),
        pg_recurring_optional_text_json(row, "frequency"),
    );
    value.insert(
        "scheduledStartDate".to_string(),
        pg_recurring_optional_text_json(row, "start_date"),
    );
    value.insert(
        "scheduledEndDate".to_string(),
        pg_recurring_optional_text_json(row, "end_date"),
    );
    value.insert("scheduledAt".to_string(), Value::Null);
    value
}
