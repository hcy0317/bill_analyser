#[tracing::instrument(level = "debug", skip_all)]
fn normalize_create_payload(fields: &BudgetRecord, now: &str) -> DbResult<BudgetRecord> {
    let mut payload = fields.clone();
    payload
        .entry("name".to_string())
        .or_insert_with(|| Value::String(String::new()));
    payload.insert(
        "sub_category".to_string(),
        Value::String(normalize_sub_category(payload.get("sub_category"))),
    );
    payload
        .entry("enabled".to_string())
        .or_insert_with(|| Value::Bool(true));
    payload
        .entry("alert_threshold".to_string())
        .or_insert_with(|| json_i64(80));
    payload
        .entry("created_at".to_string())
        .or_insert_with(|| Value::String(now.to_string()));
    payload
        .entry("updated_at".to_string())
        .or_insert_with(|| Value::String(now.to_string()));
    for field in ["category", "period_type", "amount_cents", "start_date"] {
        if missing_required_field(&payload, field) {
            return Err(DbError::InvalidOperation(format!(
                "missing required budget field: {field}"
            )));
        }
    }
    canonicalize_budget_period_type(&mut payload, None)?;
    validate_budget_amount_cents(&payload)?;
    Ok(payload)
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_update_payload(
    existing: &BudgetRecord,
    fields: &BudgetRecord,
) -> DbResult<BudgetRecord> {
    let mut payload = fields.clone();
    if payload.contains_key("sub_category") {
        payload.insert(
            "sub_category".to_string(),
            Value::String(normalize_sub_category(payload.get("sub_category"))),
        );
    } else if should_copy_existing_sub_category(&payload) {
        payload.insert(
            "sub_category".to_string(),
            Value::String(normalize_sub_category(existing.get("sub_category"))),
        );
    }
    payload
        .entry("updated_at".to_string())
        .or_insert_with(|| Value::String(now_text()));
    for key in payload.keys() {
        if !BUDGET_UPDATE_COLUMNS.contains(&key.as_str()) {
            return Err(DbError::InvalidOperation(format!(
                "invalid budget update field: {key}"
            )));
        }
    }
    if payload.contains_key("amount_cents") {
        validate_budget_amount_cents(&payload)?;
    }
    if payload.contains_key("period_type") {
        canonicalize_budget_period_type(&mut payload, None)?;
    }
    Ok(payload)
}

fn normalize_import_payload(fields: &BudgetRecord) -> DbResult<BudgetRecord> {
    let mut payload = fields.clone();
    canonicalize_budget_period_type(&mut payload, Some(BudgetPeriodKind::Monthly))?;
    payload
        .entry("category".to_string())
        .or_insert(Value::Null);
    payload
        .entry("sub_category".to_string())
        .or_insert_with(|| Value::String(String::new()));
    payload.entry("end_date".to_string()).or_insert(Value::Null);
    payload
        .entry("alert_threshold".to_string())
        .or_insert_with(|| json_i64(80));
    payload
        .entry("enabled".to_string())
        .or_insert_with(|| Value::Bool(true));
    Ok(payload)
}

fn canonicalize_budget_period_type(
    payload: &mut BudgetRecord,
    missing_default: Option<BudgetPeriodKind>,
) -> DbResult<BudgetPeriodKind> {
    let period_text = record_text(payload, "period_type");
    let trimmed = period_text.trim();
    let period_kind = if trimmed.is_empty() {
        missing_default.ok_or_else(|| {
            DbError::InvalidOperation("missing required budget field: period_type".to_string())
        })?
    } else {
        BudgetPeriodKind::parse(trimmed).map_err(DbError::InvalidOperation)?
    };
    payload.insert(
        "period_type".to_string(),
        Value::String(period_kind.as_str().to_string()),
    );
    Ok(period_kind)
}

fn validate_budget_amount_cents(payload: &BudgetRecord) -> DbResult<()> {
    record_i64(payload, "amount_cents")
        .map(|_| ())
        .ok_or_else(|| DbError::InvalidOperation("invalid budget amount_cents".to_string()))
}

fn should_copy_existing_sub_category(payload: &BudgetRecord) -> bool {
    [
        "category",
        "period_type",
        "amount_cents",
        "start_date",
        "end_date",
    ]
    .iter()
    .any(|key| payload.contains_key(*key))
}

fn budget_import_has_required_name_and_amount(budget: &BudgetRecord) -> bool {
    !missing_required_field(budget, "name") && !missing_required_field(budget, "amount_cents")
}
