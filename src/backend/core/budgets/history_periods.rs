// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和兼容 payload 在进入或离开本层时必须显式转换。


#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_budget_query_end_date(end_date: Option<&str>) -> Option<String> {
    let end_date = non_empty_str(end_date)?;
    if end_date.len() <= 10 {
        Some(format!("{end_date} 23:59:59"))
    } else {
        Some(end_date.to_string())
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn expand_forecast_history_window(
    period_type: &str,
    start_date: &str,
    end_date: &str,
    history_periods: i64,
) -> Result<BudgetPeriodRange, String> {
    let start = parse_budget_date_prefix(start_date)?;
    let end = parse_budget_date_prefix(end_date)?;
    if history_periods <= 1 {
        return Ok(BudgetPeriodRange {
            start_date: start_date.to_string(),
            end_date: format_date(end),
        });
    }
    let periods_to_expand = history_periods - 1;
    let expanded_start = match period_type {
        "daily" => start - Duration::days(periods_to_expand),
        "weekly" => start - Duration::weeks(periods_to_expand),
        "quarterly" => shift_month_start(start, -3 * periods_to_expand)?,
        "yearly" => make_date(
            start.year()
                - i32::try_from(periods_to_expand)
                    .map_err(|_| "Invalid history periods".to_string())?,
            start.month(),
            start.day(),
        )?,
        _ => shift_month_start(start, -periods_to_expand)?,
    };
    Ok(BudgetPeriodRange {
        start_date: format_date(expanded_start),
        end_date: format_date(end),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_forecast_period_key(period_type: &str, date: NaiveDate) -> String {
    #[cfg(not(coverage))]
    tracing::info!(domain = "budget", operation = "build_forecast_period_key", "business operation entered");
    match period_type {
        "daily" => format_date(date),
        "weekly" => date.format("%Y-%W").to_string(),
        "quarterly" => format!("{}-Q{}", date.year(), ((date.month() - 1) / 3) + 1),
        "yearly" => date.year().to_string(),
        _ => date.format("%Y-%m").to_string(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_parent_budget_period(
    child_period_type: &str,
    child_start_date: &str,
    parent_period_type: &str,
) -> Result<Option<BudgetPeriodRange>, String> {
    let child_start = parse_budget_date_prefix(child_start_date)?;
    match (child_period_type, parent_period_type) {
        ("monthly", "quarterly") => {
            let quarter = ((child_start.month() - 1) / 3) + 1;
            let start_month = ((quarter - 1) * 3) + 1;
            let end_month = start_month + 2;
            Ok(Some(BudgetPeriodRange {
                start_date: format_date(make_date(child_start.year(), start_month, 1)?),
                end_date: format_date(last_day_of_month(child_start.year(), end_month)?),
            }))
        }
        ("monthly", "yearly") | ("quarterly", "yearly") => Ok(Some(BudgetPeriodRange {
            start_date: format_date(make_date(child_start.year(), 1, 1)?),
            end_date: format_date(make_date(child_start.year(), 12, 31)?),
        })),
        _ => Ok(None),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn rollup_parent_amount(existing_parent_amount: f64, child_total: f64) -> f64 {
    existing_parent_amount.max(child_total)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn rollup_yearly_child_total(
    quarterly_amounts: &BTreeMap<u32, f64>,
    monthly_totals_by_quarter: &BTreeMap<u32, f64>,
) -> f64 {
    let mut total = 0.0;
    for quarter in 1..=4 {
        let quarterly = quarterly_amounts.get(&quarter).copied().unwrap_or_default();
        let monthly = monthly_totals_by_quarter
            .get(&quarter)
            .copied()
            .unwrap_or_default();
        total += quarterly.max(monthly);
    }
    total
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_budget_history_filter_summary(input: &BudgetHistoryFilterSummaryInput) -> String {
    #[cfg(not(coverage))]
    tracing::info!(domain = "budget", operation = "build_budget_history_filter_summary", "business operation entered");
    let mut fields = BTreeMap::new();
    fields.insert(
        "account_ids",
        PythonJsonValue::IntArray(sorted_ids(input.account_ids.as_deref())),
    );
    fields.insert("budget_id", PythonJsonValue::OptionalInt(input.budget_id));
    fields.insert(
        "budget_type",
        PythonJsonValue::Int(i64::from(input.budget_type)),
    );
    fields.insert(
        "category_id",
        PythonJsonValue::OptionalInt(input.category_id),
    );
    fields.insert(
        "period_type",
        PythonJsonValue::String(input.period_type.clone().unwrap_or_default()),
    );
    fields.insert(
        "tag_ids",
        PythonJsonValue::IntArray(sorted_ids(input.tag_ids.as_deref())),
    );

    let rendered = fields
        .into_iter()
        .map(|(key, value)| format!("{}: {}", python_json_string(key), value.render()))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{rendered}}}")
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn iter_budget_history_period_ranges(
    period_type: &str,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<BudgetPeriodRange>, String> {
    let start = parse_budget_date_prefix(start_date)?;
    let end = parse_budget_date_prefix(end_date)?;
    if start > end {
        return Ok(Vec::new());
    }

    let mut ranges = Vec::new();
    let mut current = match period_type {
        "weekly" => start - Duration::days(i64::from(start.weekday().num_days_from_monday())),
        "monthly" => make_date(start.year(), start.month(), 1)?,
        "quarterly" => make_date(start.year(), (((start.month() - 1) / 3) * 3) + 1, 1)?,
        "yearly" => make_date(start.year(), 1, 1)?,
        _ => start,
    };

    while current <= end {
        let range = match period_type {
            "weekly" => BudgetPeriodRange {
                start_date: format_date(current),
                end_date: format_date(current + Duration::days(6)),
            },
            "monthly" => BudgetPeriodRange {
                start_date: format_date(current),
                end_date: format_date(last_day_of_month(current.year(), current.month())?),
            },
            "quarterly" => {
                let end_month = current.month() + 2;
                BudgetPeriodRange {
                    start_date: format_date(current),
                    end_date: format_date(last_day_of_month(current.year(), end_month)?),
                }
            }
            "yearly" => BudgetPeriodRange {
                start_date: format_date(current),
                end_date: format_date(make_date(current.year(), 12, 31)?),
            },
            _ => BudgetPeriodRange {
                start_date: format_date(current),
                end_date: format_date(current),
            },
        };
        ranges.push(range);

        current = match period_type {
            "weekly" => current + Duration::weeks(1),
            "monthly" => shift_month_start(current, 1)?,
            "quarterly" => shift_month_start(current, 3)?,
            "yearly" => shift_month_start(current, 12)?,
            _ => current + Duration::days(1),
        };
    }

    Ok(ranges)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn budget_overlaps_period(
    budget_start: &str,
    budget_end: Option<&str>,
    period_start: &str,
    period_end: &str,
) -> Result<bool, String> {
    let budget_start = parse_budget_date_prefix(budget_start)?;
    let budget_end = match non_empty_str(budget_end) {
        Some(end_date) => Some(parse_budget_date_prefix(end_date)?),
        None => None,
    };
    let period_start = parse_budget_date_prefix(period_start)?;
    let period_end = parse_budget_date_prefix(period_end)?;

    Ok(budget_start <= period_end && budget_end.is_none_or(|end| end >= period_start))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_budget_history_item_from_detail(detail: &Value, period: &BudgetPeriodRange) -> Value {
    #[cfg(not(coverage))]
    tracing::info!(domain = "budget", operation = "build_budget_history_item_from_detail", "business operation entered");
    build_budget_history_item_from_detail_with_context(
        detail,
        period,
        BUDGET_TYPE_EXPENSE,
        "monthly",
        "",
    )
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_budget_history_item_from_detail_with_context(
    detail: &Value,
    period: &BudgetPeriodRange,
    budget_type: i32,
    period_type: &str,
    filter_summary: &str,
) -> Value {
    #[cfg(not(coverage))]
    tracing::info!(domain = "budget", operation = "build_budget_history_item_from_detail_with_context", "business operation entered");
    let id = value_to_i64(detail.get("id")).unwrap_or_default();
    let budget_amount = budget_item_amount(detail, "budget_amount");
    let spent_amount = budget_item_amount(detail, "spent_amount");
    let status = if spent_amount > budget_amount {
        "over_budget"
    } else {
        "within_budget"
    };

    json!({
        "id": format!("{id}_{}_{}", period.start_date, period.end_date),
        "budget_id": id,
        "period_start": period.start_date,
        "period_end": period.end_date,
        "budget_amount": round2(budget_amount),
        "spent_amount": round2(spent_amount),
        "remaining_amount": detail.get("remaining_amount").cloned().unwrap_or_else(|| json!(round2(budget_amount - spent_amount))),
        "execution_rate": detail.get("execution_rate").cloned().unwrap_or_else(|| json!(if budget_amount > 0.0 { round2((spent_amount / budget_amount) * 100.0) } else { 0.0 })),
        "status": status,
        "filter_summary": filter_summary,
        "calculated_at": "",
        "name": detail.get("name").cloned().unwrap_or_else(|| json!("")),
        "category": detail.get("category").cloned().unwrap_or_else(|| json!("")),
        "sub_category": detail.get("sub_category").cloned().unwrap_or_else(|| json!("")),
        "category_id": detail.get("category_id").cloned().unwrap_or_else(|| json!("")),
        "category_info": detail.get("category_info").cloned().unwrap_or(Value::Null),
        "type": detail.get("type").cloned().unwrap_or_else(|| json!(budget_type)),
        "period_type": detail.get("period_type").cloned().unwrap_or_else(|| json!(period_type)),
        "alert_threshold": detail.get("alert_threshold").cloned().unwrap_or_else(|| json!(80)),
        "enabled": detail.get("enabled").cloned().unwrap_or_else(|| json!(1))
    })
}
