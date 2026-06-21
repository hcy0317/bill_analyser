fn category_rule_select_sql() -> &'static str {
    r#"
        SELECT
            cr.id, cr.user_id, cr.category_id, cr.name, cr.priority,
            cr.rule_expression, cr.enabled, cr.created_at, cr.updated_at,
            c.path, c.name AS category_name, c.category_type
        FROM category_rules cr
        JOIN categories c ON cr.category_id = c.id
    "#
}

fn account_rule_select_sql() -> &'static str {
    r#"
        SELECT
            ar.id, ar.user_id, ar.account_id, ar.name, ar.priority,
            ar.rule_expression, ar.regex_enabled, ar.enabled,
            ar.match_count, ar.last_matched_at,
            ar.source, ar.source_key, ar.created_at, ar.updated_at,
            a.name AS account_name, a.account_type AS account_type, NOT a.is_active AS account_hidden
        FROM account_rules ar
        JOIN accounts a ON ar.account_id = a.id
    "#
}

fn category_rule_from_postgres_row(row: PgRow) -> DbResult<CategoryRuleRecord> {
    let path = row
        .try_get::<Option<String>, _>("path")?
        .unwrap_or_default();
    let category_name: String = row.try_get("category_name")?;
    let (main_category, sub_category) = category_names_from_path(&path, &category_name);
    let rule_expression_json: Value = row.try_get("rule_expression")?;
    let enabled: bool = row.try_get("enabled")?;
    let mut record = Map::new();
    insert_i64(&mut record, "id", row.try_get("id")?);
    insert_i64(&mut record, "user_id", row.try_get("user_id")?);
    insert_i64(
        &mut record,
        "category_id",
        row.try_get::<Option<i64>, _>("category_id")?
            .unwrap_or_default(),
    );
    insert_string(&mut record, "name", row.try_get("name")?);
    insert_i64(
        &mut record,
        "priority",
        i64::from(row.try_get::<i32, _>("priority")?),
    );
    insert_string(
        &mut record,
        "rule_expression",
        rule_expression_string(&rule_expression_json),
    );
    insert_i64(
        &mut record,
        "regex_enabled",
        bool_as_i64(rule_expression_regex_enabled(&rule_expression_json)),
    );
    insert_i64(&mut record, "enabled", bool_as_i64(enabled));
    insert_i64(&mut record, "applied_count", 0);
    record.insert("last_applied_at".to_string(), Value::Null);
    insert_timestamp(&mut record, "created_at", row.try_get("created_at")?);
    insert_timestamp(&mut record, "updated_at", row.try_get("updated_at")?);
    insert_string(&mut record, "main_category", main_category);
    insert_string(&mut record, "sub_category", sub_category);
    insert_number_or_string(
        &mut record,
        "category_type",
        row.try_get::<Option<String>, _>("category_type")?
            .as_deref()
            .unwrap_or_default(),
    );
    Ok(record)
}

fn category_rule_has_displayable_expression(record: &CategoryRuleRecord) -> bool {
    record
        .get("rule_expression")
        .and_then(Value::as_str)
        .is_some_and(|expression| !expression.trim().is_empty())
}

fn category_rule_result_has_displayable_expression(record: &DbResult<CategoryRuleRecord>) -> bool {
    match record {
        Ok(record) => category_rule_has_displayable_expression(record),
        Err(_) => true,
    }
}

fn account_rule_from_postgres_row(row: PgRow) -> DbResult<AccountRuleRecord> {
    let rule_expression_json: Value = row.try_get("rule_expression")?;
    let regex_enabled: bool = row.try_get("regex_enabled")?;
    let enabled: bool = row.try_get("enabled")?;
    let mut record = Map::new();
    insert_i64(&mut record, "id", row.try_get("id")?);
    insert_i64(&mut record, "user_id", row.try_get("user_id")?);
    insert_i64(
        &mut record,
        "account_id",
        row.try_get::<Option<i64>, _>("account_id")?
            .unwrap_or_default(),
    );
    insert_string(&mut record, "name", row.try_get("name")?);
    insert_i64(
        &mut record,
        "priority",
        i64::from(row.try_get::<i32, _>("priority")?),
    );
    insert_string(
        &mut record,
        "rule_expression",
        rule_expression_string(&rule_expression_json),
    );
    insert_i64(&mut record, "regex_enabled", bool_as_i64(regex_enabled));
    insert_i64(&mut record, "enabled", bool_as_i64(enabled));
    insert_i64(&mut record, "applied_count", 0);
    record.insert("last_applied_at".to_string(), Value::Null);
    insert_i64(&mut record, "match_count", row.try_get("match_count")?);
    insert_optional_timestamp(
        &mut record,
        "last_matched_at",
        row.try_get("last_matched_at")?,
    );
    insert_string(&mut record, "source", row.try_get("source")?);
    record.insert(
        "source_key".to_string(),
        row.try_get::<Option<String>, _>("source_key")?
            .map_or(Value::Null, Value::String),
    );
    insert_timestamp(&mut record, "created_at", row.try_get("created_at")?);
    insert_timestamp(&mut record, "updated_at", row.try_get("updated_at")?);
    insert_string(&mut record, "account_name", row.try_get("account_name")?);
    insert_number_or_string(
        &mut record,
        "account_type",
        row.try_get::<Option<String>, _>("account_type")?
            .as_deref()
            .unwrap_or_default(),
    );
    insert_i64(
        &mut record,
        "account_hidden",
        bool_as_i64(row.try_get("account_hidden")?),
    );
    Ok(record)
}

fn account_from_postgres_row(row: PgRow) -> DbResult<AccountRecord> {
    let metadata: Value = row.try_get("metadata")?;
    let mut account = Map::new();
    let balance_cents: i64 = row.try_get("balance_cents")?;
    let is_active: bool = row.try_get("is_active")?;

    insert_i64(&mut account, "id", row.try_get("id")?);
    insert_i64(&mut account, "user_id", row.try_get("user_id")?);
    insert_string(&mut account, "name", row.try_get("name")?);
    insert_number_or_string(
        &mut account,
        "type",
        row.try_get::<Option<String>, _>("account_type")?
            .as_deref()
            .unwrap_or_default(),
    );
    account.insert(
        "category".to_string(),
        metadata_value_or_default(&metadata, "category", Value::Number(Number::from(0))),
    );
    insert_string(
        &mut account,
        "currency",
        row.try_get::<Option<String>, _>("currency")?
            .unwrap_or_else(|| "CNY".to_string()),
    );
    account.insert(
        "icon".to_string(),
        metadata_value_or_default(&metadata, "icon", Value::String(String::new())),
    );
    account.insert(
        "color".to_string(),
        metadata_value_or_default(&metadata, "color", Value::String(String::new())),
    );
    account.insert(
        "balanceCents".to_string(),
        Value::Number(Number::from(balance_cents)),
    );
    let initial_balance_cents = metadata_initial_balance_cents(&metadata, balance_cents);
    account.insert(
        "initialBalanceCents".to_string(),
        Value::Number(Number::from(initial_balance_cents)),
    );
    account.insert("hidden".to_string(), Value::Bool(!is_active));
    insert_i64(
        &mut account,
        "display_order",
        i64::from(row.try_get::<i32, _>("display_order")?),
    );
    account.insert(
        "comment".to_string(),
        metadata_value_or_default(&metadata, "comment", Value::String(String::new())),
    );
    account.insert(
        "parent_id".to_string(),
        metadata_value_or_default(&metadata, "parent_id", Value::Number(Number::from(0))),
    );
    account.insert(
        "credit_card_statement_date".to_string(),
        metadata_value_or_default(&metadata, "credit_card_statement_date", Value::Null),
    );
    insert_timestamp(&mut account, "created_at", row.try_get("created_at")?);
    insert_timestamp(&mut account, "updated_at", row.try_get("updated_at")?);
    if !row
        .try_get::<Option<String>, _>("payment_method")?
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        account.insert(
            "payment_method".to_string(),
            Value::String(row.try_get::<Option<String>, _>("payment_method")?.unwrap()),
        );
    }
    Ok(account)
}

fn category_from_postgres_row(row: PgRow) -> DbResult<CategoryRecord> {
    let metadata: Value = row.try_get("metadata")?;
    let path = row
        .try_get::<Option<String>, _>("path")?
        .unwrap_or_default();
    let name: String = row.try_get("name")?;
    let (main_category, sub_category) = category_names_from_path(&path, &name);
    let is_active: bool = row.try_get("is_active")?;

    let mut category = Map::new();
    insert_i64(&mut category, "id", row.try_get("id")?);
    insert_i64(&mut category, "user_id", row.try_get("user_id")?);
    insert_number_or_string(
        &mut category,
        "type",
        row.try_get::<Option<String>, _>("category_type")?
            .as_deref()
            .unwrap_or_default(),
    );
    insert_string(&mut category, "main_category", main_category);
    insert_string(&mut category, "sub_category", sub_category);
    category.insert(
        "description".to_string(),
        metadata_value_or_default(&metadata, "description", Value::String(String::new())),
    );
    insert_i64(
        &mut category,
        "priority",
        i64::from(row.try_get::<i32, _>("display_order")?),
    );
    category.insert("hidden".to_string(), Value::Bool(!is_active));
    category.insert(
        "icon".to_string(),
        row.try_get::<Option<String>, _>("icon")?
            .map_or(Value::String(String::new()), Value::String),
    );
    category.insert(
        "color".to_string(),
        row.try_get::<Option<String>, _>("color")?
            .map_or(Value::String(String::new()), Value::String),
    );
    insert_timestamp(&mut category, "created_at", row.try_get("created_at")?);
    Ok(category)
}

fn tag_from_postgres_row(row: PgRow) -> DbResult<TagRecord> {
    let metadata: Value = row.try_get("metadata")?;
    Ok(TagRecord {
        id: row.try_get("id")?,
        user_id: row.try_get("user_id")?,
        name: row.try_get("name")?,
        color: row.try_get("color")?,
        icon: metadata
            .get("icon")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        display_order: i64::from(row.try_get::<i32, _>("display_order")?),
        hidden: metadata_bool(&metadata, "hidden").map_or(0, i64::from),
        created_at: timestamp_to_string(row.try_get("created_at")?),
        updated_at: timestamp_to_string(row.try_get("updated_at")?),
    })
}

fn template_from_postgres_row(row: PgRow) -> DbResult<TemplateRecord> {
    let template_type: i32 = row.try_get("template_type")?;
    let tag_ids: Value = row.try_get("tag_ids")?;
    let mut template = Map::new();
    template.insert(
        "id".to_string(),
        Value::String(row.try_get::<i64, _>("id")?.to_string()),
    );
    template.insert(
        "templateType".to_string(),
        Value::Number(Number::from(i64::from(template_type))),
    );
    template.insert("name".to_string(), Value::String(row.try_get("name")?));
    template.insert(
        "description".to_string(),
        optional_string_value(row.try_get("description")?),
    );
    template.insert(
        "type".to_string(),
        Value::Number(Number::from(normalize_template_transaction_type(
            row.try_get::<Option<String>, _>("transaction_type")?
                .as_deref(),
        ))),
    );
    template.insert(
        "categoryId".to_string(),
        string_or_default(row.try_get("category_id")?, ""),
    );
    template.insert(
        "sourceAccountId".to_string(),
        string_or_default(row.try_get("source_account_id")?, "0"),
    );
    template.insert(
        "destinationAccountId".to_string(),
        string_or_default(row.try_get("destination_account_id")?, "0"),
    );
    template.insert(
        "sourceAmountCents".to_string(),
        Value::Number(Number::from(
            row.try_get::<i64, _>("source_amount_minor_units")?,
        )),
    );
    template.insert(
        "destinationAmountCents".to_string(),
        Value::Number(Number::from(
            row.try_get::<i64, _>("destination_amount_minor_units")?,
        )),
    );
    template.insert(
        "hideAmount".to_string(),
        Value::Bool(row.try_get("hide_amount")?),
    );
    template.insert(
        "tagIds".to_string(),
        tag_ids
            .as_array()
            .cloned()
            .map_or(Value::Array(Vec::new()), Value::Array),
    );
    template.insert(
        "comment".to_string(),
        optional_string_value(row.try_get("comment")?),
    );
    template.insert("editable".to_string(), Value::Bool(true));
    template.insert(
        "displayOrder".to_string(),
        Value::Number(Number::from(i64::from(
            row.try_get::<i32, _>("display_order")?,
        ))),
    );
    template.insert("hidden".to_string(), Value::Bool(row.try_get("hidden")?));
    template.insert(
        "scheduledFrequencyType".to_string(),
        scheduled_value(
            template_type,
            row.try_get::<Option<i32>, _>("scheduled_frequency_type")?
                .map(|value| Value::Number(Number::from(i64::from(value)))),
        ),
    );
    template.insert(
        "scheduledFrequency".to_string(),
        scheduled_value(
            template_type,
            row.try_get::<Option<String>, _>("scheduled_frequency")?
                .map(Value::String),
        ),
    );
    template.insert(
        "scheduledStartDate".to_string(),
        scheduled_value(
            template_type,
            row.try_get::<Option<String>, _>("scheduled_start_date")?
                .map(Value::String),
        ),
    );
    template.insert(
        "scheduledEndDate".to_string(),
        scheduled_value(
            template_type,
            row.try_get::<Option<String>, _>("scheduled_end_date")?
                .map(Value::String),
        ),
    );
    template.insert("scheduledAt".to_string(), Value::Null);
    template.insert("enabled".to_string(), Value::Bool(row.try_get("enabled")?));
    template.insert(
        "autoCreate".to_string(),
        Value::Bool(row.try_get("auto_create")?),
    );
    template.insert(
        "utcOffset".to_string(),
        Value::Number(Number::from(i64::from(
            row.try_get::<i32, _>("utc_offset")?,
        ))),
    );
    Ok(template)
}

fn category_names_from_path(path: &str, name: &str) -> (String, String) {
    let parts = path
        .split('/')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    match parts.as_slice() {
        [] => (name.to_string(), String::new()),
        [main] => ((*main).to_string(), String::new()),
        [main, rest @ ..] => ((*main).to_string(), rest.join("/")),
    }
}

fn metadata_value_or_default(metadata: &Value, key: &str, default: Value) -> Value {
    metadata.get(key).cloned().unwrap_or(default)
}

fn metadata_bool(metadata: &Value, key: &str) -> Option<bool> {
    metadata.get(key).and_then(|value| match value {
        Value::Bool(flag) => Some(*flag),
        Value::Number(number) => number.as_i64().map(|value| value != 0),
        Value::String(text) => {
            let normalized = text.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "1" | "true" | "yes" | "on" => Some(true),
                "0" | "false" | "no" | "off" | "" => Some(false),
                _ => None,
            }
        }
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    })
}

fn insert_i64(record: &mut Map<String, Value>, key: &str, value: i64) {
    record.insert(key.to_string(), Value::Number(Number::from(value)));
}

fn insert_string(record: &mut Map<String, Value>, key: &str, value: String) {
    record.insert(key.to_string(), Value::String(value));
}

fn insert_number_or_string(record: &mut Map<String, Value>, key: &str, value: &str) {
    let trimmed = value.trim();
    if let Ok(number) = trimmed.parse::<i64>() {
        record.insert(key.to_string(), Value::Number(Number::from(number)));
    } else {
        record.insert(key.to_string(), Value::String(trimmed.to_string()));
    }
}

fn insert_timestamp(record: &mut Map<String, Value>, key: &str, value: DateTime<Utc>) {
    record.insert(key.to_string(), Value::String(timestamp_to_string(value)));
}

fn insert_optional_timestamp(
    record: &mut Map<String, Value>,
    key: &str,
    value: Option<DateTime<Utc>>,
) {
    record.insert(
        key.to_string(),
        value.map_or(Value::Null, |value| {
            Value::String(timestamp_to_string(value))
        }),
    );
}

fn timestamp_to_string(value: DateTime<Utc>) -> String {
    value.to_rfc3339()
}

fn optional_string_value(value: Option<String>) -> Value {
    value.map_or_else(|| Value::String(String::new()), Value::String)
}

fn string_or_default(value: Option<String>, default: &str) -> Value {
    Value::String(value.unwrap_or_else(|| default.to_string()))
}

fn scheduled_value(template_type: i32, value: Option<Value>) -> Value {
    if template_type == 2 {
        value.unwrap_or(Value::Null)
    } else {
        Value::Null
    }
}

fn normalize_template_transaction_type(value: Option<&str>) -> i64 {
    match value.unwrap_or_default().trim().to_lowercase().as_str() {
        "2" | "income" | "收入" => 2,
        "4" | "transfer" | "转账" => 4,
        "5" | "investment" | "投资" => 5,
        _ => 3,
    }
}

fn template_select_sql(where_clause: &str) -> String {
    format!(
        r#"
        SELECT id, user_id, template_type, name, description, transaction_type,
            category_id, source_account_id, destination_account_id,
            source_amount_minor_units::BIGINT AS source_amount_minor_units,
            destination_amount_minor_units::BIGINT AS destination_amount_minor_units,
            hide_amount,
            tag_ids, comment, scheduled_frequency_type, scheduled_frequency,
            scheduled_start_date, scheduled_end_date, scheduled_next_date,
            enabled, auto_create, display_order, hidden, utc_offset, created_at, updated_at
        FROM transaction_templates
        {where_clause}
        "#
    )
}

fn metadata_initial_balance_cents(metadata: &Value, fallback_cents: i64) -> i64 {
    metadata
        .get("initial_balance_cents")
        .and_then(strict_cents_value)
        .unwrap_or(fallback_cents)
}
