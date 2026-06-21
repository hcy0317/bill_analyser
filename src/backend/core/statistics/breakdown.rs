#[tracing::instrument(level = "debug", skip_all)]

pub fn build_category_pie_data(bills: &[StatisticsBillInput]) -> Vec<NameValueStatisticItem> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_category_pie_data",
        "business operation entered"
    );
    let mut totals: BTreeMap<String, i64> = BTreeMap::new();
    for bill in bills {
        let category = bill.main_category.clone();
        *totals.entry(category).or_insert(0) += bill.amount_cents.abs();
    }

    let mut result = totals
        .into_iter()
        .map(|(name, total)| NameValueStatisticItem {
            name,
            value_cents: total,
        })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| {
        right
            .value_cents
            .cmp(&left.value_cents)
            .then_with(|| left.name.cmp(&right.name))
    });
    result
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_top_merchants_data(
    bills: &[StatisticsBillInput],
    limit: usize,
) -> Vec<TopMerchantStatisticItem> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_top_merchants_data",
        "business operation entered"
    );
    let mut totals: BTreeMap<String, (i64, usize)> = BTreeMap::new();
    for bill in bills {
        let merchant = if bill.counterparty.trim().is_empty() {
            "未知商家".to_string()
        } else {
            bill.counterparty.clone()
        };
        let entry = totals.entry(merchant).or_insert((0, 0));
        entry.0 += bill.amount_cents.abs();
        entry.1 += 1;
    }

    let mut result = totals
        .into_iter()
        .map(|(name, (amount, count))| TopMerchantStatisticItem {
            name,
            amount_cents: amount,
            count,
        })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| {
        right
            .amount_cents
            .cmp(&left.amount_cents)
            .then_with(|| left.name.cmp(&right.name))
    });
    result.truncate(limit);
    result
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn parse_transaction_amount_period_query(period_query: &str) -> Option<(String, i64, i64)> {
    let parts = period_query.split('_').collect::<Vec<_>>();
    if parts.len() != 3 {
        return None;
    }
    let start_time = parts[1].parse::<i64>().ok()?;
    let end_time = parts[2].parse::<i64>().ok()?;
    Some((parts[0].to_string(), start_time, end_time))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_transaction_amount_period_result(
    start_time: i64,
    end_time: i64,
    bills: &[StatisticsBillInput],
) -> TransactionAmountPeriodResult {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_transaction_amount_period_result",
        "business operation entered"
    );
    let total_income = bills
        .iter()
        .filter(|bill| is_income_type(&bill.bill_type))
        .map(|bill| bill.amount_cents.abs())
        .sum::<i64>();
    let total_expense = bills
        .iter()
        .filter(|bill| is_expense_type(&bill.bill_type))
        .map(|bill| bill.amount_cents.abs())
        .sum::<i64>();

    TransactionAmountPeriodResult {
        start_time,
        end_time,
        amounts: vec![TransactionAmountBucket {
            currency: "CNY".to_string(),
            income_amount_cents: total_income,
            expense_amount_cents: total_expense,
        }],
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_transaction_amounts_response(
    period_results: &BTreeMap<String, TransactionAmountPeriodResult>,
) -> Value {
    json!({"success": true, "result": period_results})
}
