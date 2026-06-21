#[tracing::instrument(level = "debug", skip_all)]

fn build_duplicate_charge_anomalies(bills: &[StatisticsBillInput]) -> Vec<Value> {
    let mut expense_bills = bills
        .iter()
        .filter(|bill| is_expense_type(&bill.bill_type))
        .collect::<Vec<_>>();
    expense_bills.sort_by_key(|bill| bill.date.clone());

    let mut anomalies = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, bill_a) in expense_bills.iter().enumerate() {
        for bill_b in expense_bills.iter().skip(index + 1).take(19) {
            let Some(date_a) = parse_bill_date_prefix(&bill_a.date) else {
                continue;
            };
            let Some(date_b) = parse_bill_date_prefix(&bill_b.date) else {
                continue;
            };
            if (date_b - date_a).num_days().abs() > 3 {
                break;
            }
            let amount_a = bill_a.amount_cents.abs();
            let amount_b = bill_b.amount_cents.abs();
            let counterparty_a = bill_a.counterparty.trim().to_lowercase();
            let counterparty_b = bill_b.counterparty.trim().to_lowercase();
            if amount_a > 0
                && amount_a == amount_b
                && !counterparty_a.is_empty()
                && counterparty_a == counterparty_b
            {
                let pair_key = format!(
                    "{}_{}",
                    bill_a.id.map_or_else(String::new, |id| id.to_string()),
                    bill_b.id.map_or_else(String::new, |id| id.to_string())
                );
                if seen.insert(pair_key) {
                    anomalies.push(json!({
                        "type": "duplicate_charge",
                        "severity": "info",
                        "billIds": [bill_a.id, bill_b.id],
                        "dates": [format_date(date_a), format_date(date_b)],
                        "amountCents": amount_a,
                        "counterparty": bill_a.counterparty,
                        "message": format!(
                            "疑似重复扣款: {} ¥{:.2} ({} & {})",
                            bill_a.counterparty,
                            display_amount_from_cents(amount_a),
                            format_date(date_a),
                            format_date(date_b)
                        ),
                    }));
                }
            }
        }
    }
    anomalies
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_category_spike_anomalies(bills: &[StatisticsBillInput]) -> Vec<Value> {
    let mut monthly: BTreeMap<String, BTreeMap<String, i64>> = BTreeMap::new();
    for bill in bills.iter().filter(|bill| is_expense_type(&bill.bill_type)) {
        let month = bill.date.chars().take(7).collect::<String>();
        if month.len() != 7 {
            continue;
        }
        *monthly
            .entry(month)
            .or_default()
            .entry(main_category_or_uncategorized(bill))
            .or_insert(0) += bill.amount_cents.abs();
    }
    if monthly.len() < 3 {
        return Vec::new();
    }
    let mut months = monthly.keys().cloned().collect::<Vec<_>>();
    months.sort();
    let latest = months.last().cloned().unwrap_or_default();
    let previous = &months[..months.len() - 1];

    let mut anomalies = Vec::new();
    for (category, latest_total) in monthly.get(&latest).into_iter().flat_map(BTreeMap::iter) {
        let previous_totals = previous
            .iter()
            .map(|month| {
                monthly
                    .get(month)
                    .and_then(|items| items.get(category))
                    .copied()
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>();
        let avg_prev = average_cents(&previous_totals);
        if avg_prev > 5_000.0 && (*latest_total as f64) > avg_prev * 2.0 {
            anomalies.push(json!({
                "type": "category_spike",
                "severity": "warning",
                "month": latest,
                "category": category,
                "currentAmountCents": *latest_total,
                "averageAmountCents": avg_prev.round() as i64,
                "ratio": round_one_decimal((*latest_total as f64) / avg_prev),
                "message": format!(
                    "{} 的 {} 支出 ¥{:.2} 是历史平均 ¥{:.2} 的 {:.1} 倍",
                    latest,
                    category,
                    display_amount_from_cents(*latest_total),
                    display_amount_from_cents(avg_prev.round() as i64),
                    (*latest_total as f64) / avg_prev
                ),
            }));
        }
    }
    anomalies
}
