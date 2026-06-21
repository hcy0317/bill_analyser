fn strict_cents_value(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok())),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => None,
    }
}

fn category_select_sql(where_clause: &str) -> String {
    format!(
        r#"
        SELECT id, user_id, parent_id, name, category_type, path, icon, color,
            display_order, is_active, metadata, created_at
        FROM categories
        {where_clause}
        "#
    )
}

async fn next_template_display_order(
    pool: &PostgresPool,
    user_id: i64,
    template_type: i64,
) -> DbResult<i64> {
    let row = sqlx::query(
        r#"
        SELECT COALESCE(MAX(display_order), 0)::BIGINT AS max_order
        FROM transaction_templates
        WHERE user_id = $1 AND template_type = $2
        "#,
    )
    .bind(user_id)
    .bind(i64_to_i32(template_type))
    .fetch_one(pool)
    .await?;
    Ok(row.try_get::<i64, _>("max_order")? + 1)
}

async fn insert_postgres_account(
    transaction: &mut Transaction<'_, Postgres>,
    payload: &Value,
    user_id: i64,
    parent_id: Option<i64>,
) -> DbResult<i64> {
    let name = value_text(payload.get("name"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| DbError::InvalidOperation("account name is required".to_string()))?;
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE user_id = $1 AND name = $2)",
    )
    .bind(user_id)
    .bind(&name)
    .fetch_one(&mut **transaction)
    .await?;
    if exists {
        return Err(DbError::InvalidOperation(format!(
            "account name already exists: {name}"
        )));
    }
    let metadata = account_metadata_from_payload(None, payload)?;
    let balance_cents = optional_strict_minor_units(
        payload
            .get("balanceCents")
            .or_else(|| payload.get("balance_cents")),
        "balanceCents",
    )?
    .unwrap_or_default();
    let row = sqlx::query(
        r#"
        INSERT INTO accounts (
            user_id, name, account_type, currency, balance_cents, is_active,
            display_order, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
    "#,
    )
    .bind(user_id)
    .bind(name)
    .bind(value_text(payload.get("type")))
    .bind(value_text(payload.get("currency")).unwrap_or_else(|| "CNY".to_string()))
    .bind(balance_cents)
    .bind(!payload.get("hidden").is_some_and(value_truthy))
    .bind(i64_to_i32(
        int_value(payload.get("display_order")).unwrap_or_default(),
    ))
    .bind(metadata_with_parent_id(metadata, parent_id))
    .fetch_one(&mut **transaction)
    .await?;
    let account_id: i64 = row.try_get("id")?;

    if let Some(sub_accounts) = payload.get("subAccounts").and_then(Value::as_array) {
        for sub_account in sub_accounts {
            Box::pin(insert_postgres_account(
                transaction,
                sub_account,
                user_id,
                Some(account_id),
            ))
            .await?;
        }
    }

    Ok(account_id)
}

#[derive(Debug)]
struct TemplateValues {
    name: String,
    description: Option<String>,
    transaction_type: Option<String>,
    category_id: Option<String>,
    source_account_id: Option<String>,
    destination_account_id: Option<String>,
    source_amount_minor_units: i64,
    destination_amount_minor_units: i64,
    hide_amount: bool,
    tag_ids: Value,
    comment: Option<String>,
    scheduled_frequency_type: Option<i64>,
    scheduled_frequency: Option<String>,
    scheduled_start_date: Option<String>,
    scheduled_end_date: Option<String>,
    scheduled_next_date: Option<String>,
    enabled: bool,
    auto_create: bool,
    display_order: i64,
    hidden: bool,
    utc_offset: i64,
}

fn template_values_from_payload(
    payload: &Value,
    template_type: i64,
    create_display_order: i64,
    existing: Option<&TemplateRecord>,
) -> DbResult<TemplateValues> {
    let display_order = payload
        .get("displayOrder")
        .and_then(|value| int_value(Some(value)))
        .or_else(|| existing.and_then(|record| int_value(record.get("displayOrder"))))
        .unwrap_or(create_display_order);
    let scheduled = template_type == 2;
    Ok(TemplateValues {
        name: value_text(payload.get("name"))
            .or_else(|| existing.and_then(|record| value_text(record.get("name"))))
            .unwrap_or_default(),
        description: optional_text_field(payload, existing, "description"),
        transaction_type: optional_text_field(payload, existing, "type"),
        category_id: optional_text_field(payload, existing, "categoryId"),
        source_account_id: optional_text_field(payload, existing, "sourceAccountId")
            .or_else(|| Some("0".to_string())),
        destination_account_id: optional_text_field(payload, existing, "destinationAccountId")
            .or_else(|| Some("0".to_string())),
        source_amount_minor_units: optional_strict_minor_units(
            payload.get("sourceAmountCents"),
            "sourceAmountCents",
        )?
        .or_else(|| {
            existing
                .and_then(|record| record.get("sourceAmountCents"))
                .and_then(strict_cents_value)
        })
        .unwrap_or_default(),
        destination_amount_minor_units: optional_strict_minor_units(
            payload.get("destinationAmountCents"),
            "destinationAmountCents",
        )?
        .or_else(|| {
            existing
                .and_then(|record| record.get("destinationAmountCents"))
                .and_then(strict_cents_value)
        })
        .unwrap_or_default(),
        hide_amount: payload
            .get("hideAmount")
            .map(value_truthy)
            .or_else(|| existing.and_then(|record| record.get("hideAmount").map(value_truthy)))
            .unwrap_or(false),
        tag_ids: payload
            .get("tagIds")
            .cloned()
            .or_else(|| existing.and_then(|record| record.get("tagIds").cloned()))
            .filter(Value::is_array)
            .unwrap_or_else(|| Value::Array(Vec::new())),
        comment: optional_text_field(payload, existing, "comment"),
        scheduled_frequency_type: scheduled
            .then(|| {
                payload
                    .get("scheduledFrequencyType")
                    .and_then(|value| int_value(Some(value)))
                    .or_else(|| {
                        existing
                            .and_then(|record| record.get("scheduledFrequencyType"))
                            .and_then(|value| int_value(Some(value)))
                    })
            })
            .flatten(),
        scheduled_frequency: scheduled
            .then(|| optional_text_field(payload, existing, "scheduledFrequency"))
            .flatten(),
        scheduled_start_date: scheduled
            .then(|| optional_text_field(payload, existing, "scheduledStartDate"))
            .flatten(),
        scheduled_end_date: scheduled
            .then(|| optional_text_field(payload, existing, "scheduledEndDate"))
            .flatten(),
        scheduled_next_date: scheduled
            .then(|| {
                optional_text_field(payload, existing, "scheduledNextDate")
                    .or_else(|| optional_text_field(payload, existing, "scheduledStartDate"))
            })
            .flatten(),
        enabled: payload
            .get("enabled")
            .map(value_truthy)
            .or_else(|| existing.and_then(|record| record.get("enabled").map(value_truthy)))
            .unwrap_or(true),
        auto_create: payload
            .get("autoCreate")
            .map(value_truthy)
            .or_else(|| existing.and_then(|record| record.get("autoCreate").map(value_truthy)))
            .unwrap_or(false),
        display_order,
        hidden: payload
            .get("hidden")
            .map(value_truthy)
            .or_else(|| existing.and_then(|record| record.get("hidden").map(value_truthy)))
            .unwrap_or(false),
        utc_offset: payload
            .get("utcOffset")
            .and_then(|value| int_value(Some(value)))
            .or_else(|| existing.and_then(|record| int_value(record.get("utcOffset"))))
            .unwrap_or_default(),
    })
}

fn optional_text_field(
    payload: &Value,
    existing: Option<&TemplateRecord>,
    key: &str,
) -> Option<String> {
    if payload.get(key).is_some() {
        return value_text(payload.get(key));
    }
    existing.and_then(|record| value_text(record.get(key)))
}

fn tag_metadata_from_payload(existing: Option<&Value>, payload: &Value) -> Value {
    let mut metadata = existing
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if payload.get("icon").is_some() || existing.is_none() {
        metadata.insert(
            "icon".to_string(),
            value_text(payload.get("icon"))
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
    }
    if payload.get("hidden").is_some() || existing.is_none() {
        metadata.insert(
            "hidden".to_string(),
            Value::Bool(payload.get("hidden").is_some_and(value_truthy)),
        );
    }
    Value::Object(metadata)
}
