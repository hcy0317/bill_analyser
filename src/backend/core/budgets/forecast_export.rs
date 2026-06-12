// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。


#[tracing::instrument(level = "debug", skip_all)]
pub fn calculate_forecast_amount_cents(amounts_cents: &[i64], strategy: &str) -> Option<i64> {
    if amounts_cents.is_empty() {
        return None;
    }
    let values = if strategy == "moving_average" {
        let window_start = amounts_cents.len().saturating_sub(3);
        &amounts_cents[window_start..]
    } else {
        amounts_cents
    };
    average_cents(values)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn calculate_forecast_backtest_mape(amounts_cents: &[i64], strategy: &str) -> Option<f64> {
    let mut errors = Vec::new();
    for index in 1..amounts_cents.len() {
        let actual = amounts_cents[index];
        if actual <= 0 {
            continue;
        }
        let predicted = calculate_forecast_amount_cents(&amounts_cents[..index], strategy)?;
        errors.push(((actual - predicted).abs() as f64 / actual as f64) * 100.0);
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
pub fn resolve_forecast_trend(amounts_cents: &[i64]) -> &'static str {
    if amounts_cents.len() < 2 {
        return "stable";
    }
    let latest = amounts_cents[amounts_cents.len() - 1] as f64;
    let baseline =
        average_cents(&amounts_cents[..amounts_cents.len() - 1]).unwrap_or_default() as f64;
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
pub fn resolve_forecast_budget_amount_cents(
    primary_amount_cents: i64,
    sub_total_cents: i64,
) -> i64 {
    if primary_amount_cents > 0 {
        primary_amount_cents
    } else {
        sub_total_cents
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_budget_forecast_item(
    category: &str,
    category_info: Value,
    amounts_cents: &[i64],
    current_spent_cents: i64,
    primary_budget_amount_cents: i64,
    sub_budget_total_cents: i64,
    strategy: &str,
) -> Value {
    #[cfg(not(coverage))]
    tracing::info!(domain = "budget", operation = "build_budget_forecast_item", "business operation entered");
    build_budget_forecast_item_from_input(BudgetForecastItemInput {
        category,
        category_info,
        amounts_cents,
        current_spent_cents,
        primary_budget_amount_cents,
        sub_budget_total_cents,
        strategy,
        period_count: amounts_cents.len(),
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
    let total_amount_cents = input.amounts_cents.iter().sum::<i64>();
    let average_amount_cents =
        calculate_forecast_amount_cents(input.amounts_cents, "historical_average")
            .unwrap_or_default();
    let forecast_amount_cents =
        calculate_forecast_amount_cents(input.amounts_cents, normalized_strategy)
            .unwrap_or_default();
    let budget_amount_cents = resolve_forecast_budget_amount_cents(
        input.primary_budget_amount_cents,
        input.sub_budget_total_cents,
    );
    let backtest_mape = calculate_forecast_backtest_mape(input.amounts_cents, normalized_strategy);
    let periods = input
        .amounts_cents
        .iter()
        .enumerate()
        .map(|(index, amount_cents)| {
            let period = input
                .period_labels
                .and_then(|labels| labels.get(index))
                .cloned()
                .unwrap_or_else(|| (index + 1).to_string());
            json!({"period": period, "amount_cents": amount_cents})
        })
        .collect::<Vec<_>>();

    json!({
        "category": input.category,
        "category_info": input.category_info,
        "total_amount_cents": total_amount_cents,
        "average_amount_cents": average_amount_cents,
        "period_count": input.period_count,
        "sample_periods": input.amounts_cents.len(),
        "current_spent_cents": input.current_spent_cents,
        "budget_amount_cents": budget_amount_cents,
        "forecast_amount_cents": forecast_amount_cents,
        "projected_over_budget": forecast_amount_cents > budget_amount_cents && budget_amount_cents > 0,
        "forecast_strategy": normalized_strategy,
        "strategy_explanation": forecast_strategy_explanation(normalized_strategy, input.amounts_cents.len()),
        "backtest_mape": backtest_mape,
        "confidence": resolve_forecast_confidence(backtest_mape),
        "trend": resolve_forecast_trend(input.amounts_cents),
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
        "amount_cents": field_or_null(row, "amount_cents"),
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
