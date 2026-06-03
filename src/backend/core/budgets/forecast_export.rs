// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。


#[tracing::instrument(level = "debug", skip_all)]
pub fn calculate_forecast_amount(amounts: &[f64], strategy: &str) -> Option<f64> {
    if amounts.is_empty() {
        return None;
    }
    let values = if strategy == "moving_average" {
        let window_start = amounts.len().saturating_sub(3);
        &amounts[window_start..]
    } else {
        amounts
    };
    average(values).map(round2)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn calculate_forecast_backtest_mape(amounts: &[f64], strategy: &str) -> Option<f64> {
    let mut errors = Vec::new();
    for index in 1..amounts.len() {
        let actual = amounts[index];
        if actual <= 0.0 {
            continue;
        }
        let predicted = calculate_forecast_amount(&amounts[..index], strategy)?;
        errors.push(((actual - predicted).abs() / actual) * 100.0);
    }
    average(&errors).map(round2)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_forecast_confidence(backtest_mape: Option<f64>) -> &'static str {
    match backtest_mape {
        Some(value) if value <= 10.0 => "high",
        Some(value) if value <= 20.0 => "medium",
        _ => "low",
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_forecast_trend(amounts: &[f64]) -> &'static str {
    if amounts.len() < 2 {
        return "stable";
    }
    let latest = amounts[amounts.len() - 1];
    let baseline = average(&amounts[..amounts.len() - 1]).unwrap_or_default();
    if baseline <= 0.0 {
        return "stable";
    }
    if latest > baseline * 1.05 {
        "up"
    } else if latest < baseline * 0.95 {
        "down"
    } else {
        "stable"
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_forecast_budget_amount(primary_amount: f64, sub_total: f64) -> f64 {
    if primary_amount > 0.0 {
        round2(primary_amount)
    } else {
        round2(sub_total)
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_budget_forecast_item(
    category: &str,
    category_info: Value,
    amounts: &[f64],
    current_spent: f64,
    primary_budget_amount: f64,
    sub_budget_total: f64,
    strategy: &str,
) -> Value {
    #[cfg(not(coverage))]
    tracing::info!(domain = "budget", operation = "build_budget_forecast_item", "business operation entered");
    build_budget_forecast_item_from_input(BudgetForecastItemInput {
        category,
        category_info,
        amounts,
        current_spent,
        primary_budget_amount,
        sub_budget_total,
        strategy,
        period_count: amounts.len(),
        period_labels: None,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_budget_forecast_item_from_input(input: BudgetForecastItemInput<'_>) -> Value {
    #[cfg(not(coverage))]
    tracing::info!(domain = "budget", operation = "build_budget_forecast_item_from_input", "business operation entered");
    let normalized_strategy = if input.strategy.trim().is_empty() {
        "historical_average"
    } else {
        input.strategy
    };
    let total_amount = round2(input.amounts.iter().sum());
    let average_amount =
        calculate_forecast_amount(input.amounts, "historical_average").unwrap_or(0.0);
    let forecast_amount =
        calculate_forecast_amount(input.amounts, normalized_strategy).unwrap_or(0.0);
    let budget_amount =
        resolve_forecast_budget_amount(input.primary_budget_amount, input.sub_budget_total);
    let backtest_mape = calculate_forecast_backtest_mape(input.amounts, normalized_strategy);
    let periods = input
        .amounts
        .iter()
        .enumerate()
        .map(|(index, amount)| {
            let period = input
                .period_labels
                .and_then(|labels| labels.get(index))
                .cloned()
                .unwrap_or_else(|| (index + 1).to_string());
            json!({"period": period, "amount": round2(*amount)})
        })
        .collect::<Vec<_>>();

    json!({
        "category": input.category,
        "category_info": input.category_info,
        "total_amount": total_amount,
        "average_amount": average_amount,
        "period_count": input.period_count,
        "sample_periods": input.amounts.len(),
        "current_spent": round2(input.current_spent),
        "budget_amount": budget_amount,
        "forecast_amount": forecast_amount,
        "projected_over_budget": forecast_amount > budget_amount && budget_amount > 0.0,
        "forecast_strategy": normalized_strategy,
        "strategy_explanation": forecast_strategy_explanation(normalized_strategy, input.amounts.len()),
        "backtest_mape": backtest_mape,
        "confidence": resolve_forecast_confidence(backtest_mape),
        "trend": resolve_forecast_trend(input.amounts),
        "periods": periods
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_budget_export_item(row: &Value) -> Value {
    #[cfg(not(coverage))]
    tracing::info!(domain = "budget", operation = "build_budget_export_item", "business operation entered");
    json!({
        "name": field_or_null(row, "name"),
        "category": field_or_null(row, "category"),
        "sub_category": field_or_null(row, "sub_category"),
        "period_type": field_or_null(row, "period_type"),
        "amount": field_or_null(row, "amount"),
        "start_date": field_or_null(row, "start_date"),
        "end_date": field_or_null(row, "end_date"),
        "alert_threshold": field_or_null(row, "alert_threshold"),
        "enabled": field_or_null(row, "enabled")
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_budget_export_response(rows: &[Value]) -> Value {
    json!({
        "success": true,
        "result": rows.iter().map(build_budget_export_item).collect::<Vec<_>>()
    })
}
