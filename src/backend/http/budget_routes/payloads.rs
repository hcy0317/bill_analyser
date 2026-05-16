
fn filters_from_query(query: &BudgetListQuery) -> RouteResult<BudgetFilters> {
    Ok(BudgetFilters {
        period_type: non_empty_string(query.period_type.as_ref()),
        enabled: query.enabled.as_deref().and_then(parse_enabled),
        category: non_empty_string(query.category.as_ref()),
        budget_type: match query
            .budget_type
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            Some(value) => Some(parse_i32(value, "budget_type")?),
            None => None,
        },
    })
}

fn budget_scope_from_execution_query(
    query: &BudgetExecutionQuery,
) -> RouteResult<bill_analyser_core::budgets::BudgetPeriodScope> {
    let input = BudgetPeriodScopeInput {
        budget_type: parse_optional_i32(query.budget_type.as_deref(), "budget_type")?,
        period_type: query.period_type.clone(),
        start_date: non_empty_string(query.start_date.as_ref()),
        end_date: non_empty_string(query.end_date.as_ref()),
        year: parse_optional_i32(query.year.as_deref(), "year")?,
        month: parse_optional_u32(query.month.as_deref(), "month")?,
        quarter: parse_optional_u32(query.quarter.as_deref(), "quarter")?,
    };
    build_budget_period_scope(&input, Utc::now().date_naive())
        .map_err(|error| Box::new(bad_request(error)) as Box<Response>)
}

fn budget_scope_from_forecast_query(
    query: &BudgetForecastQuery,
) -> RouteResult<(bill_analyser_core::budgets::BudgetPeriodScope, i32)> {
    let months_history =
        parse_optional_i32(query.months_history.as_deref(), "months_history")?.unwrap_or(6);
    let input = BudgetPeriodScopeInput {
        budget_type: parse_optional_i32(query.budget_type.as_deref(), "budget_type")?,
        period_type: query.period_type.clone(),
        start_date: non_empty_string(query.start_date.as_ref()),
        end_date: non_empty_string(query.end_date.as_ref()),
        year: parse_optional_i32(query.year.as_deref(), "year")?,
        month: parse_optional_u32(query.month.as_deref(), "month")?,
        quarter: parse_optional_u32(query.quarter.as_deref(), "quarter")?,
    };
    validate_budget_period_args(&input, Some(i64::from(months_history)))
        .map_err(|error| Box::new(bad_request(error)))?;
    let scope = build_budget_period_scope(&input, Utc::now().date_naive())
        .map_err(|error| Box::new(bad_request(error)))?;
    Ok((scope, months_history))
}

fn budget_scope_from_payload(
    payload: &Map<String, Value>,
) -> RouteResult<bill_analyser_core::budgets::BudgetPeriodScope> {
    let input = BudgetPeriodScopeInput {
        budget_type: parse_optional_i32_value(payload.get("budget_type"), "budget_type")?,
        period_type: non_empty_value_string(payload.get("period_type")),
        start_date: non_empty_value_string(payload.get("start_date")),
        end_date: non_empty_value_string(payload.get("end_date")),
        year: parse_optional_i32_value(payload.get("year"), "year")?,
        month: parse_optional_u32_value(payload.get("month"), "month")?,
        quarter: parse_optional_u32_value(payload.get("quarter"), "quarter")?,
    };
    build_budget_period_scope(&input, Utc::now().date_naive())
        .map_err(|error| Box::new(bad_request(error)) as Box<Response>)
}

fn execution_filters_from_query(
    query: &BudgetExecutionQuery,
    scope: &bill_analyser_core::budgets::BudgetPeriodScope,
) -> RouteResult<BudgetExecutionFilters> {
    Ok(BudgetExecutionFilters {
        budget_type: scope.budget_type,
        period_type: Some(scope.period_type.clone()),
        start_date: Some(scope.start_date.clone()),
        end_date: Some(scope.end_date.clone()),
        budget_id: parse_optional_i64(query.budget_id.as_deref(), "budget_id")?,
        category_id: parse_optional_i64(query.category_id.as_deref(), "category_id")?,
        account_ids: parse_budget_csv_int_list(query.account_ids.as_deref(), "account_ids")
            .map_err(|error| Box::new(bad_request(error)))?,
        tag_ids: parse_budget_csv_int_list(query.tag_ids.as_deref(), "tag_ids")
            .map_err(|error| Box::new(bad_request(error)))?,
    })
}

fn execution_filters_from_payload(
    payload: &Map<String, Value>,
    scope: &bill_analyser_core::budgets::BudgetPeriodScope,
) -> RouteResult<BudgetExecutionFilters> {
    let account_ids = parse_budget_json_int_list(payload.get("account_ids"), "account_ids")
        .map_err(|error| Box::new(bad_request(error)))?;
    let tag_ids = parse_budget_json_int_list(payload.get("tag_ids"), "tag_ids")
        .map_err(|error| Box::new(bad_request(error)))?;
    Ok(BudgetExecutionFilters {
        budget_type: scope.budget_type,
        period_type: Some(scope.period_type.clone()),
        start_date: Some(scope.start_date.clone()),
        end_date: Some(scope.end_date.clone()),
        budget_id: parse_optional_i64_value(payload.get("budget_id"), "budget_id")?,
        category_id: parse_optional_i64_value(payload.get("category_id"), "category_id")?,
        account_ids: (!account_ids.is_empty()).then_some(account_ids),
        tag_ids: (!tag_ids.is_empty()).then_some(tag_ids),
    })
}

fn import_budget_records_from_payload(payload: &Value) -> RouteResult<Vec<BudgetRecord>> {
    let Some(items) = payload.as_array().filter(|items| !items.is_empty()) else {
        return Err(Box::new(invalid_budget_import_data_response()));
    };
    let mut records = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let Some(object) = item.as_object() else {
            return Err(Box::new(bad_request(format!(
                "Invalid budget item at index {index}"
            ))));
        };
        validate_import_budget_record(object, index)?;
        records.push(object.clone());
    }
    Ok(records)
}

fn validate_import_budget_record(payload: &BudgetRecord, index: usize) -> RouteResult<()> {
    for field in ["period_type", "amount", "start_date"] {
        if !payload.contains_key(field) {
            return Err(Box::new(bad_request(format!(
                "Missing required field at index {index}: {field}"
            ))));
        }
    }
    if value_string(payload.get("category")).is_none() {
        return Err(Box::new(bad_request(format!(
            "Missing required field at index {index}: category"
        ))));
    }

    let period_type = value_string(payload.get("period_type")).unwrap_or_default();
    validate_budget_period_args(
        &BudgetPeriodScopeInput {
            period_type: Some(period_type),
            ..BudgetPeriodScopeInput::default()
        },
        None,
    )
    .map_err(|error| Box::new(bad_request(error)))?;
    validate_budget_date_range(
        value_string(payload.get("start_date")).as_deref(),
        value_string(payload.get("end_date")).as_deref(),
    )
    .map_err(|error| Box::new(bad_request(error)))?;
    Ok(())
}

fn create_fields_from_payload(payload: &Value) -> RouteResult<BudgetRecord> {
    let mut fields = payload_object(payload)?.clone();
    for field in ["period_type", "amount", "start_date"] {
        if missing_required_field(&fields, field) {
            return Err(Box::new(bad_request(format!(
                "Missing required field: {field}"
            ))));
        }
    }
    if missing_required_field(&fields, "category") {
        return Err(Box::new(bad_request("Missing required field: category")));
    }
    let period_type = value_string(fields.get("period_type")).unwrap_or_default();
    validate_budget_period_args(
        &BudgetPeriodScopeInput {
            period_type: Some(period_type.clone()),
            ..BudgetPeriodScopeInput::default()
        },
        None,
    )
    .map_err(|error| Box::new(bad_request(error)))?;
    validate_budget_date_range(
        value_string(fields.get("start_date")).as_deref(),
        value_string(fields.get("end_date")).as_deref(),
    )
    .map_err(|error| Box::new(bad_request(error)))?;
    fields
        .entry("name".to_string())
        .or_insert_with(|| Value::String(String::new()));
    fields
        .entry("enabled".to_string())
        .or_insert_with(|| Value::Bool(true));
    fields
        .entry("alert_threshold".to_string())
        .or_insert_with(|| json!(80));
    Ok(fields)
}

fn update_fields_from_payload(
    payload: &Value,
    existing_budget: &BudgetRecord,
) -> RouteResult<BudgetRecord> {
    let mut fields = payload_object(payload)?.clone();
    if let Some(period_type) = fields
        .get("period_type")
        .and_then(|value| value_string(Some(value)))
    {
        validate_budget_period_args(
            &BudgetPeriodScopeInput {
                period_type: Some(period_type),
                ..BudgetPeriodScopeInput::default()
            },
            None,
        )
        .map_err(|error| Box::new(bad_request(error)))?;
    }
    let start_date = value_string(fields.get("start_date"))
        .or_else(|| value_string(existing_budget.get("start_date")));
    let end_date = value_string(fields.get("end_date"))
        .or_else(|| value_string(existing_budget.get("end_date")));
    validate_budget_date_range(start_date.as_deref(), end_date.as_deref())
        .map_err(|error| Box::new(bad_request(error)))?;
    fields.insert("updated_at".to_string(), Value::String(now_text()));
    Ok(fields)
}

fn payload_object(payload: &Value) -> RouteResult<&Map<String, Value>> {
    payload
        .as_object()
        .filter(|object| !object.is_empty())
        .ok_or_else(|| Box::new(bad_request("No data provided")))
}

fn missing_required_field(payload: &BudgetRecord, field: &str) -> bool {
    payload
        .get(field)
        .is_none_or(|value| value.is_null() || value.as_str().is_some_and(|text| text.is_empty()))
}

fn parse_i32(value: &str, field_name: &str) -> RouteResult<i32> {
    value.trim().parse::<i32>().map_err(|_| {
        Box::new(bad_request(format!(
            "Invalid integer for {field_name}: {value}"
        )))
    })
}

fn parse_optional_i32(value: Option<&str>, field_name: &str) -> RouteResult<Option<i32>> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| parse_i32(value, field_name))
        .transpose()
}

fn parse_optional_i32_value(value: Option<&Value>, field_name: &str) -> RouteResult<Option<i32>> {
    let value = value_string(value);
    parse_optional_i32(value.as_deref(), field_name)
}

fn parse_optional_i64(value: Option<&str>, field_name: &str) -> RouteResult<Option<i64>> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value.trim().parse::<i64>().map_err(|_| {
                Box::new(bad_request(format!(
                    "Invalid integer for {field_name}: {value}"
                ))) as Box<Response>
            })
        })
        .transpose()
}

fn parse_optional_i64_value(value: Option<&Value>, field_name: &str) -> RouteResult<Option<i64>> {
    let value = value_string(value);
    parse_optional_i64(value.as_deref(), field_name)
}

fn parse_optional_u32(value: Option<&str>, field_name: &str) -> RouteResult<Option<u32>> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value.trim().parse::<u32>().map_err(|_| {
                Box::new(bad_request(format!(
                    "Invalid integer for {field_name}: {value}"
                ))) as Box<Response>
            })
        })
        .transpose()
}

fn parse_optional_u32_value(value: Option<&Value>, field_name: &str) -> RouteResult<Option<u32>> {
    let value = value_string(value);
    parse_optional_u32(value.as_deref(), field_name)
}

fn parse_enabled(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

fn value_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(text.trim().to_string()),
        Some(Value::Number(number)) => Some(number.to_string()),
        Some(Value::Bool(value)) => Some(value.to_string()),
        _ => None,
    }
    .filter(|value| !value.is_empty())
}

fn non_empty_value_string(value: Option<&Value>) -> Option<String> {
    value_string(value)
}

fn forecast_amount(item: &Value) -> f64 {
    item.get("forecast_amount")
        .and_then(Value::as_f64)
        .unwrap_or_default()
}

fn round2(value: f64) -> f64 {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded.abs() < 0.005 {
        0.0
    } else {
        rounded
    }
}

fn non_empty_string(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}
