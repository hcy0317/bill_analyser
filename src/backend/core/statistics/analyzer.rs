#[tracing::instrument(level = "debug", skip_all)]

pub fn build_statistics_trend_points(analyzer_result: &Value) -> Vec<StatisticsTrendPoint> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_statistics_trend_points",
        "business operation entered"
    );
    analyzer_result
        .get("trends")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|trend| StatisticsTrendPoint {
            date: trend
                .get("period")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            income_cents: value_as_i64(
                trend
                    .get("incomeCents")
                    .or_else(|| trend.get("income_cents")),
            )
            .unwrap_or(0),
            expense_cents: value_as_i64(
                trend
                    .get("expenseCents")
                    .or_else(|| trend.get("expense_cents")),
            )
            .unwrap_or(0),
            net_cents: value_as_i64(trend.get("netCents").or_else(|| trend.get("net_cents")))
                .unwrap_or(0),
        })
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_statistics_trend_response(analyzer_result: &Value) -> Value {
    json!({"success": true, "data": build_statistics_trend_points(analyzer_result)})
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn statistics_analyzer_period_range(
    period: &str,
    today: NaiveDate,
) -> StatisticsAnalyzerPeriodRange {
    let start = match period {
        "month" => first_day(today.year(), today.month()),
        "quarter" => {
            let start_month = ((today.month() - 1) / 3) * 3 + 1;
            first_day(today.year(), start_month)
        }
        "year" => first_day(today.year(), 1),
        _ => today - Duration::days(30),
    };
    let end = match period {
        "month" => add_months(start, 1),
        "quarter" => add_months(start, 3),
        "year" => first_day(today.year() + 1, 1),
        _ => today,
    };
    StatisticsAnalyzerPeriodRange {
        start_date: format_date(start),
        end_date: format_date(end),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_statistics_analyzer_report(
    period: &str,
    range: &StatisticsAnalyzerPeriodRange,
    bills: &[StatisticsBillInput],
    generated_at: &str,
) -> Value {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_statistics_analyzer_report",
        "business operation entered"
    );
    if bills.is_empty() {
        return empty_statistics_analyzer_report(generated_at);
    }

    json!({
        "period": period,
        "start_date": range.start_date,
        "end_date": range.end_date,
        "total_records": bills.len(),
        "summary": statistics_analyzer_summary(bills),
        "by_category": statistics_analyzer_by_category(bills),
        "by_type": statistics_analyzer_by_type(bills),
        "trend": statistics_analyzer_report_trend(bills, period),
        "top_expenses": statistics_analyzer_top_bills(bills, "支出", 10, true),
        "top_income": statistics_analyzer_top_bills(bills, "收入", 10, false),
        "generated_at": generated_at,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_statistics_analyzer_trends_result(
    period: &str,
    category: Option<&str>,
    buckets: &[StatisticsAnalyzerTrendBucket],
) -> Value {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_statistics_analyzer_trends_result",
        "business operation entered"
    );
    json!({
        "trends": buckets,
        "period": period,
        "category": category,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_statistics_analyzer_trend_bucket(
    period: &str,
    bills: &[StatisticsBillInput],
) -> StatisticsAnalyzerTrendBucket {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_statistics_analyzer_trend_bucket",
        "business operation entered"
    );
    let income_cents = bills
        .iter()
        .filter(|bill| is_income_type(&bill.bill_type))
        .map(|bill| bill.amount_cents)
        .sum::<i64>();
    let expense_cents = bills
        .iter()
        .filter(|bill| is_expense_type(&bill.bill_type))
        .map(|bill| bill.amount_cents)
        .sum::<i64>();
    StatisticsAnalyzerTrendBucket {
        period: period.to_string(),
        income_cents,
        expense_cents,
        net_cents: income_cents + expense_cents,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_statistics_analyzer_comparison_result(
    period: &str,
    compare_type: &str,
    bills: &[StatisticsBillInput],
) -> Value {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_statistics_analyzer_comparison_result",
        "business operation entered"
    );
    let comparison = if compare_type == "category" {
        let mut categories: BTreeMap<String, (i64, i64, usize)> = BTreeMap::new();
        for bill in bills {
            let entry = categories
                .entry(main_category_or_uncategorized(bill))
                .or_insert((0, 0, 0));
            if is_income_type(&bill.bill_type) {
                entry.0 += bill.amount_cents;
            } else if is_expense_type(&bill.bill_type) {
                entry.1 += bill.amount_cents;
            }
            entry.2 += 1;
        }
        let mut rows = categories
            .into_iter()
            .map(|(name, (income_cents, expense_cents, count))| {
                json!({
                    "name": name,
                    "income_cents": income_cents,
                    "expense_cents": expense_cents,
                    "count": count,
                    "net_cents": income_cents + expense_cents,
                })
            })
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| {
            let left_expense = value_as_i64(left.get("expense_cents")).unwrap_or(0).abs();
            let right_expense = value_as_i64(right.get("expense_cents")).unwrap_or(0).abs();
            right_expense.cmp(&left_expense)
        });
        rows
    } else {
        Vec::new()
    };

    json!({
        "comparison": comparison,
        "period": period,
        "compare_type": compare_type,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_statistics_analyzer_category_result(
    period: &str,
    main_category: Option<&str>,
    bills: &[StatisticsBillInput],
) -> Value {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_statistics_analyzer_category_result",
        "business operation entered"
    );
    let mut sub_categories: BTreeMap<String, (i64, usize)> = BTreeMap::new();
    let mut total_cents = 0_i64;
    for bill in bills.iter().filter(|bill| is_expense_type(&bill.bill_type)) {
        let amount = bill.amount_cents;
        let entry = sub_categories
            .entry(if bill.sub_category.is_empty() {
                "其他".to_string()
            } else {
                bill.sub_category.clone()
            })
            .or_insert((0, 0));
        entry.0 += amount;
        entry.1 += 1;
        total_cents += amount;
    }

    let mut rows = sub_categories
        .into_iter()
        .map(|(sub_category, (amount_cents, count))| {
            let total_abs_cents = total_cents.abs();
            let percentage = if total_abs_cents > 0 {
                round_money((amount_cents.abs() as f64 / total_abs_cents as f64) * 100.0)
            } else {
                0.0
            };
            json!({
                "sub_category": sub_category,
                "amount_cents": amount_cents,
                "count": count,
                "percentage": percentage,
                "avg_amount_cents": average_total_cents(amount_cents, count),
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        let left_amount = value_as_i64(left.get("amount_cents")).unwrap_or(0).abs();
        let right_amount = value_as_i64(right.get("amount_cents")).unwrap_or(0).abs();
        right_amount.cmp(&left_amount)
    });

    json!({
        "main_category": main_category,
        "period": period,
        "total_amount_cents": total_cents,
        "sub_categories": rows,
    })
}
