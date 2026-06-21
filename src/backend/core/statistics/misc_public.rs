#[tracing::instrument(level = "debug", skip_all)]

pub fn normalize_chinese_currency_name(value: &str) -> String {
    let text = value
        .trim()
        .replace('（', "(")
        .replace('）', ")")
        .split('(')
        .next()
        .unwrap_or_default()
        .replace(' ', "");
    chinese_currency_name_map()
        .get(text.as_str())
        .unwrap_or(&"")
        .to_string()
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn extract_numeric_values(values: &[&str]) -> Vec<f64> {
    values
        .iter()
        .filter_map(|value| {
            let text = value.trim();
            if text.is_empty() || matches!(text, "-" | "--" | "nan" | "NaN") || text.contains(':') {
                return None;
            }
            let normalized = text.replace(',', "");
            if normalized
                .chars()
                .all(|ch| ch.is_ascii_digit() || matches!(ch, '-' | '.'))
                && normalized.parse::<f64>().is_ok()
            {
                normalized.parse::<f64>().ok()
            } else {
                None
            }
        })
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_overview_result_from_report(report: &Value) -> Value {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_overview_result_from_report",
        "business operation entered"
    );
    let summary = report.get("summary").unwrap_or(&Value::Null);
    let total_income_cents = value_as_i64(summary.get("total_income_cents")).unwrap_or(0);
    let total_expense_cents = value_as_i64(summary.get("total_expense_cents"))
        .unwrap_or(0)
        .abs();
    json!({
        "total_income_cents": total_income_cents,
        "total_expense_cents": total_expense_cents,
        "net_income_cents": total_income_cents - total_expense_cents,
        "bill_count": report.get("total_records").and_then(Value::as_i64).unwrap_or(0),
        "by_category": report.get("by_category").cloned().unwrap_or_else(|| json!({})),
        "by_type": report.get("by_type").cloned().unwrap_or_else(|| json!({})),
        "top_income": report.get("top_income").cloned().unwrap_or_else(|| json!([])),
        "top_expenses": report.get("top_expenses").cloned().unwrap_or_else(|| json!([])),
        "period": report.get("period").and_then(Value::as_str).unwrap_or_default(),
        "start_date": report.get("start_date").and_then(Value::as_str).unwrap_or_default(),
        "end_date": report.get("end_date").and_then(Value::as_str).unwrap_or_default(),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_statistics_report_chart_plan(report: &Value) -> Vec<String> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_statistics_report_chart_plan",
        "business operation entered"
    );
    let mut chart_ids = Vec::new();
    if value_is_non_empty_array(report.get("trend")) {
        chart_ids.push("trend".to_string());
    }
    if value_is_non_empty_object(report.get("by_category")) {
        chart_ids.push("category_pie".to_string());
    }
    if value_is_non_empty_array(report.get("top_expenses")) {
        chart_ids.push("top_expenses".to_string());
    }
    if value_is_non_empty_object(report.get("summary")) {
        chart_ids.push("comparison".to_string());
    }
    if value_is_non_empty_object(Some(report)) {
        chart_ids.push("dashboard".to_string());
    }
    chart_ids
}
