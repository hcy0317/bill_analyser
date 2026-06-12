// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。


#[tracing::instrument(level = "debug", skip_all)]
pub fn calculate_avg_backtest_mape(items: &[Value]) -> Option<f64> {
    let mut values = Vec::new();
    for item in items {
        if let Some(value) = item.get("backtest_mape").and_then(value_to_f64) {
            values.push(value);
        }
    }
    average(&values).map(round2)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn select_budget_detail_items(items: &[Value]) -> Vec<Value> {
    select_budget_items(items, true)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn select_budget_summary_items(items: &[Value]) -> Vec<Value> {
    select_budget_items(items, false)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_budget_execution_summary(items: &[Value]) -> Value {
    #[cfg(not(coverage))]
    tracing::info!(domain = "budget", operation = "build_budget_execution_summary", "business operation entered");
    let selected = select_budget_summary_items(items);
    let total_budget_cents = selected
        .iter()
        .map(|item| budget_item_amount_cents(item, "budget_amount_cents"))
        .sum::<i64>();
    let total_spent_cents = selected
        .iter()
        .map(|item| budget_item_amount_cents(item, "spent_amount_cents"))
        .sum::<i64>();
    let total_remaining_cents = total_budget_cents - total_spent_cents;
    let overall_execution_rate = if total_budget_cents > 0 {
        round2((total_spent_cents as f64 / total_budget_cents as f64) * 100.0)
    } else {
        0.0
    };

    json!({
        "total_budget_cents": total_budget_cents,
        "total_spent_cents": total_spent_cents,
        "total_remaining_cents": total_remaining_cents,
        "overall_execution_rate": overall_execution_rate,
        "count": selected.len()
    })
}
