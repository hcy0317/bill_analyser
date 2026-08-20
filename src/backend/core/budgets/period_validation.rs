// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。


#[tracing::instrument(level = "debug", skip_all)]
pub fn is_valid_budget_period_type(period_type: &str) -> bool {
    BudgetPeriodKind::parse(period_type).is_ok()
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn validate_budget_period_args(
    input: &BudgetPeriodScopeInput,
    months_history: Option<i64>,
) -> Result<(), String> {
    budget_period_kind(input)?;
    validate_budget_period_shape(input, months_history)
}

fn validate_budget_period_shape(
    input: &BudgetPeriodScopeInput,
    months_history: Option<i64>,
) -> Result<(), String> {
    if input.month.is_some_and(|month| !(1..=12).contains(&month)) {
        return Err("month must be between 1 and 12".to_string());
    }
    if input
        .quarter
        .is_some_and(|quarter| !(1..=4).contains(&quarter))
    {
        return Err("quarter must be between 1 and 4".to_string());
    }
    if months_history.is_some_and(|history| history < 1) {
        return Err("months_history must be >= 1".to_string());
    }

    Ok(())
}

fn budget_period_kind(input: &BudgetPeriodScopeInput) -> Result<BudgetPeriodKind, String> {
    BudgetPeriodKind::parse(input.period_type.as_deref().unwrap_or("monthly").trim())
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn validate_budget_date_range(
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> Result<(), String> {
    let Some(start_date) = non_empty_str(start_date) else {
        return Ok(());
    };
    let Some(end_date) = non_empty_str(end_date) else {
        return Ok(());
    };

    let start = parse_budget_date_prefix(start_date)?;
    let end = parse_budget_date_prefix(end_date)?;
    if start > end {
        return Err("Invalid date range: start_date must be <= end_date".to_string());
    }
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_budget_period_range(
    input: &BudgetPeriodScopeInput,
    today: NaiveDate,
) -> Result<BudgetPeriodRange, String> {
    let period_kind = budget_period_kind(input)?;
    validate_budget_period_shape(input, None)?;
    resolve_budget_period_range_for_kind(input, today, period_kind)
}

fn resolve_budget_period_range_for_kind(
    input: &BudgetPeriodScopeInput,
    today: NaiveDate,
    period_kind: BudgetPeriodKind,
) -> Result<BudgetPeriodRange, String> {
    if non_empty_str(input.start_date.as_deref()).is_some()
        && non_empty_str(input.end_date.as_deref()).is_some()
    {
        validate_budget_date_range(input.start_date.as_deref(), input.end_date.as_deref())?;
        return Ok(BudgetPeriodRange {
            start_date: input.start_date.clone().unwrap_or_default(),
            end_date: input.end_date.clone().unwrap_or_default(),
        });
    }

    let year = input.year.unwrap_or(today.year());
    let anchor = match period_kind {
        BudgetPeriodKind::Daily | BudgetPeriodKind::Weekly => today,
        BudgetPeriodKind::Monthly => {
            let month = input.month.unwrap_or(today.month());
            make_date(year, month, 1)?
        }
        BudgetPeriodKind::Quarterly => {
            let quarter = input
                .quarter
                .unwrap_or_else(|| ((today.month() - 1) / 3) + 1);
            let start_month = ((quarter - 1) * 3) + 1;
            make_date(year, start_month, 1)?
        }
        BudgetPeriodKind::Yearly => make_date(year, 1, 1)?,
    };
    period_kind.containing(anchor)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_budget_period_scope(
    input: &BudgetPeriodScopeInput,
    today: NaiveDate,
) -> Result<BudgetPeriodScope, String> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "budget", operation = "build_budget_period_scope", "business operation entered");
    let period_kind = budget_period_kind(input)?;
    validate_budget_period_shape(input, None)?;
    let range = resolve_budget_period_range_for_kind(input, today, period_kind)?;
    Ok(BudgetPeriodScope {
        budget_type: input.budget_type.unwrap_or(BUDGET_TYPE_EXPENSE),
        period_type: period_kind.as_str().to_string(),
        start_date: range.start_date,
        end_date: range.end_date,
        year: input.year,
        month: input.month,
        quarter: input.quarter,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn parse_budget_csv_int_list(
    raw: Option<&str>,
    field_name: &str,
) -> Result<Option<Vec<i64>>, String> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    if raw.trim().is_empty() {
        return Ok(None);
    }

    let mut values = Vec::new();
    for item in raw.split(',') {
        let cleaned = item.trim();
        if cleaned.is_empty() {
            continue;
        }
        values.push(parse_budget_i64(
            cleaned,
            &format!("Invalid integer in {field_name}"),
        )?);
    }

    if values.is_empty() {
        Ok(None)
    } else {
        Ok(Some(values))
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn parse_budget_json_int_list(
    raw: Option<&Value>,
    field_name: &str,
) -> Result<Vec<i64>, String> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    let Value::Array(items) = raw else {
        return Err(format!(
            "Invalid array for {field_name}: expected JSON array"
        ));
    };

    let mut values = Vec::new();
    for item in items {
        let cleaned = json_int_candidate_text(item);
        if cleaned.is_empty() {
            continue;
        }
        values.push(parse_budget_i64(
            &cleaned,
            &format!("Invalid integer in {field_name}"),
        )?);
    }
    Ok(values)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn validate_import_budget_item(item: &Value, index: usize) -> Result<(), String> {
    let Value::Object(object) = item else {
        return Err(format!("第{index}条预算格式无效"));
    };

    for required_field in ["period_type", "amount_cents", "start_date"] {
        if !object.contains_key(required_field) || object[required_field].is_null() {
            return Err(format!("第{index}条预算缺少必填字段: {required_field}"));
        }
    }
    if string_field(object, "category").trim().is_empty() {
        return Err(format!("第{index}条预算缺少必填字段: category"));
    }

    let period_type = string_field(object, "period_type");
    if !is_valid_budget_period_type(period_type.trim()) {
        return Err(format!("第{index}条预算 period_type 无效: {period_type}"));
    }
    validate_budget_date_range(
        Some(&string_field(object, "start_date")),
        non_empty_str(Some(&string_field(object, "end_date"))),
    )?;
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn calculate_budget_period_progress(
    start_date: &str,
    end_date: &str,
    today: NaiveDate,
) -> Result<BudgetPeriodProgress, String> {
    let start = parse_budget_date_prefix(start_date)?;
    let end = parse_budget_date_prefix(end_date)?;
    let total_days = (end - start).num_days() + 1;

    if today < start {
        return Ok(BudgetPeriodProgress {
            elapsed_days: 0,
            remaining_days: total_days,
        });
    }
    if today > end {
        return Ok(BudgetPeriodProgress {
            elapsed_days: total_days,
            remaining_days: 0,
        });
    }

    Ok(BudgetPeriodProgress {
        elapsed_days: (today - start).num_days() + 1,
        remaining_days: (end - today).num_days(),
    })
}
