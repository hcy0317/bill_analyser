#[tracing::instrument(level = "debug", skip_all)]

pub fn build_category_statistics_items(
    bills: &[StatisticsBillInput],
    categories: &[StatisticsCategoryInput],
    accounts: &[StatisticsAccountInput],
) -> Vec<CategoryStatisticItem> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_category_statistics_items",
        "business operation entered"
    );
    let category_name_to_id = build_category_name_to_id(categories);
    let account_name_to_id = build_account_name_to_id(accounts);
    let valid_account_ids = accounts
        .iter()
        .map(|account| account.id.to_string())
        .collect::<BTreeSet<_>>();

    let mut totals: BTreeMap<(String, String), i64> = BTreeMap::new();
    for bill in bills {
        let category_id = category_name_to_id
            .get(&category_key(&bill.main_category, &bill.sub_category))
            .cloned()
            .unwrap_or_else(|| "0".to_string());
        let account_id =
            resolve_statistics_account_id(bill, &valid_account_ids, &account_name_to_id);
        let amount = signed_statistics_amount_cents(bill);
        *totals.entry((category_id, account_id)).or_insert(0) += amount;
    }

    totals
        .into_iter()
        .map(
            |((category_id, account_id), amount)| CategoryStatisticItem {
                category_id,
                account_id,
                amount_cents: amount,
            },
        )
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_category_statistics_response(
    start_time: i64,
    end_time: i64,
    items: &[CategoryStatisticItem],
) -> Value {
    json!({
        "success": true,
        "result": {
            "startTime": start_time,
            "endTime": end_time,
            "items": items,
        }
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_category_trend_statistics(
    bills: &[StatisticsBillInput],
    categories: &[StatisticsCategoryInput],
    accounts: &[StatisticsAccountInput],
    range: &StatisticsYearMonthRange,
) -> Vec<CategoryTrendBucket> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_category_trend_statistics",
        "business operation entered"
    );
    let category_name_to_id = build_category_name_to_id(categories);
    let account_name_to_id = build_account_name_to_id(accounts);
    let valid_account_ids = accounts
        .iter()
        .map(|account| account.id.to_string())
        .collect::<BTreeSet<_>>();

    let mut monthly: BTreeMap<(i32, u32), BTreeMap<(String, String), i64>> = BTreeMap::new();
    for bill in bills {
        let Some(date) = parse_bill_date_prefix(&bill.date) else {
            continue;
        };
        let month_key = (date.year(), date.month());
        let category_id = category_name_to_id
            .get(&category_key(&bill.main_category, &bill.sub_category))
            .cloned()
            .unwrap_or_else(|| "0".to_string());
        let account_id =
            resolve_statistics_account_id(bill, &valid_account_ids, &account_name_to_id);
        let amount = signed_statistics_amount_cents(bill);
        *monthly
            .entry(month_key)
            .or_default()
            .entry((category_id, account_id))
            .or_insert(0) += amount;
    }

    iter_year_months(
        range.start_year,
        range.start_month,
        range.end_year,
        range.end_month,
    )
    .into_iter()
    .map(|(year, month)| {
        let items = monthly
            .remove(&(year, month))
            .unwrap_or_default()
            .into_iter()
            .map(
                |((category_id, account_id), amount)| CategoryStatisticItem {
                    category_id,
                    account_id,
                    amount_cents: amount,
                },
            )
            .collect();
        CategoryTrendBucket { year, month, items }
    })
    .collect()
}
