#[derive(Debug)]
struct CategoryValues {
    main_category: String,
    sub_category: String,
    name: String,
    category_type: Option<String>,
    path: String,
    icon: Option<String>,
    color: Option<String>,
    display_order: i64,
    hidden: bool,
    metadata: Value,
}

fn category_values_from_payload(
    payload: &Value,
    existing: Option<&CategoryRecord>,
) -> CategoryValues {
    let main_category = value_text(payload.get("main_category"))
        .or_else(|| existing.and_then(|record| value_text(record.get("main_category"))))
        .unwrap_or_default();
    let sub_category = value_text(payload.get("sub_category"))
        .or_else(|| existing.and_then(|record| value_text(record.get("sub_category"))))
        .unwrap_or_default();
    let name = if sub_category.trim().is_empty() {
        main_category.trim().to_string()
    } else {
        sub_category.trim().to_string()
    };
    let category_type = value_text(payload.get("type"))
        .or_else(|| existing.and_then(|record| value_text(record.get("type"))));
    let display_order = int_value(payload.get("priority"))
        .or_else(|| int_value(payload.get("displayOrder")))
        .or_else(|| existing.and_then(|record| int_value(record.get("priority"))))
        .unwrap_or_default();
    let hidden = payload
        .get("hidden")
        .map(value_truthy)
        .or_else(|| existing.and_then(|record| record.get("hidden").map(value_truthy)))
        .unwrap_or(false);
    let icon = optional_text_for_category(payload, existing, "icon");
    let color = optional_text_for_category(payload, existing, "color");
    let metadata = category_metadata_from_payload(existing, payload);
    CategoryValues {
        path: category_path(&main_category, &sub_category),
        main_category,
        sub_category,
        name,
        category_type,
        icon,
        color,
        display_order,
        hidden,
        metadata,
    }
}

async fn parent_category_id_for_values(
    pool: &PostgresPool,
    user_id: i64,
    values: &CategoryValues,
) -> DbResult<Option<i64>> {
    if values.sub_category.trim().is_empty() {
        return Ok(None);
    }
    let row = sqlx::query("SELECT id FROM categories WHERE user_id = $1 AND path = $2")
        .bind(user_id)
        .bind(values.main_category.trim())
        .fetch_optional(pool)
        .await?;
    row.map(|row| row.try_get("id"))
        .transpose()
        .map_err(Into::into)
}

fn category_path(main_category: &str, sub_category: &str) -> String {
    let main_category = main_category.trim();
    let sub_category = sub_category.trim();
    if sub_category.is_empty() {
        main_category.to_string()
    } else {
        format!("{main_category}/{sub_category}")
    }
}

fn category_metadata_from_payload(existing: Option<&CategoryRecord>, payload: &Value) -> Value {
    let mut metadata = Map::new();
    if let Some(existing) = existing {
        for key in ["description"] {
            if let Some(value) = existing.get(key) {
                metadata.insert(key.to_string(), value.clone());
            }
        }
    }
    for key in ["description"] {
        if let Some(value) = payload.get(key) {
            metadata.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(metadata)
}

fn optional_text_for_category(
    payload: &Value,
    existing: Option<&CategoryRecord>,
    key: &str,
) -> Option<String> {
    if payload.get(key).is_some() {
        return value_text(payload.get(key)).filter(|value| !value.is_empty());
    }
    existing
        .and_then(|record| value_text(record.get(key)))
        .filter(|value| !value.is_empty())
}

fn value_text(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Number(number)) => Some(number.to_string()),
        Some(Value::Bool(flag)) => Some(flag.to_string()),
        Some(Value::Null) | None => None,
        Some(value @ (Value::Array(_) | Value::Object(_))) => Some(value.to_string()),
    }
}

fn int_value(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(number)) => number.as_i64(),
        Some(Value::String(text)) => text.trim().parse::<i64>().ok(),
        Some(Value::Bool(flag)) => Some(i64::from(*flag)),
        Some(Value::Null) | None | Some(Value::Array(_) | Value::Object(_)) => None,
    }
}

fn value_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_i64().unwrap_or_default() != 0,
        Value::String(text) => {
            let normalized = text.trim().to_ascii_lowercase();
            !matches!(normalized.as_str(), "" | "0" | "false" | "none" | "null")
        }
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
        Value::Null => false,
    }
}
