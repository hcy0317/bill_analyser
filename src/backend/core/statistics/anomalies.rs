/// 生成统计洞察异常摘要，合并大额交易、重复扣款和分类突增信号。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_insight_anomaly_summary(
    bills: &[StatisticsBillInput],
    analyzed_months: u32,
    start_date: &str,
    end_date: &str,
) -> Value {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_insight_anomaly_summary",
        "business operation entered"
    );
    let mut anomalies = Vec::<Value>::new();
    let mut category_amounts: BTreeMap<String, Vec<i64>> = BTreeMap::new();

    for bill in bills {
        if is_expense_type(&bill.bill_type) {
            let amount = bill.amount_cents.abs();
            if amount > 0 {
                category_amounts
                    .entry(main_category_or_uncategorized(bill))
                    .or_default()
                    .push(amount);
            }
        }
    }

    for bill in bills {
        if !is_expense_type(&bill.bill_type) {
            continue;
        }
        let category = main_category_or_uncategorized(bill);
        let amount = bill.amount_cents.abs();
        let Some(amounts) = category_amounts.get(&category) else {
            continue;
        };
        let avg = average_cents(amounts);
        if avg > 0.0 && (amount as f64) > avg * 3.0 && amount > 10_000 {
            anomalies.push(json!({
                "type": "large_transaction",
                "severity": "warning",
                "billId": bill.id,
                "date": bill.date.chars().take(10).collect::<String>(),
                "amountCents": amount,
                "category": category,
                "averageCents": avg.round() as i64,
                "ratio": round_one_decimal((amount as f64) / avg),
                "description": first_non_empty(&bill.description, &bill.counterparty),
                "message": format!(
                    "此笔交易金额 ¥{:.2} 是 {} 分类平均值 ¥{:.2} 的 {:.1} 倍",
                    display_amount_from_cents(amount),
                    category,
                    display_amount_from_cents(avg.round() as i64),
                    (amount as f64) / avg
                ),
            }));
        }
    }

    anomalies.extend(build_duplicate_charge_anomalies(bills));
    anomalies.extend(build_category_spike_anomalies(bills));
    anomalies.sort_by(|left, right| {
        let severity_order = |value: &Value| match value.get("severity").and_then(Value::as_str) {
            Some("error") => 0,
            Some("warning") => 1,
            Some("info") => 2,
            _ => 9,
        };
        let left_date = left.get("date").and_then(Value::as_str).unwrap_or_default();
        let right_date = right
            .get("date")
            .and_then(Value::as_str)
            .unwrap_or_default();
        (severity_order(left), left_date).cmp(&(severity_order(right), right_date))
    });
    let total_count = anomalies.len();
    anomalies.truncate(100);

    json!({
        "success": true,
        "data": {
            "anomalies": anomalies,
            "totalCount": total_count,
            "analyzedBills": bills.len(),
            "analyzedMonths": analyzed_months,
            "startDate": start_date,
            "endDate": end_date,
        }
    })
}
