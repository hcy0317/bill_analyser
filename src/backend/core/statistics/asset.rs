#[tracing::instrument(level = "debug", skip_all)]

pub fn build_asset_trends(
    bills: &[StatisticsBillInput],
    accounts: &[StatisticsAccountInput],
    balances_before_date_cents: &BTreeMap<i64, i64>,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Vec<AssetTrendDay> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_asset_trends",
        "business operation entered"
    );
    let mut current_balances: BTreeMap<i64, i64> = BTreeMap::new();
    for account in accounts {
        let initial = account.initial_balance_cents;
        let history = balances_before_date_cents
            .get(&account.id)
            .copied()
            .unwrap_or_default();
        current_balances.insert(account.id, initial + history);
    }

    let mut bills_by_date: BTreeMap<String, Vec<&StatisticsBillInput>> = BTreeMap::new();
    for bill in bills {
        if let Some(date) = parse_bill_date_prefix(&bill.date) {
            bills_by_date
                .entry(format_date(date))
                .or_default()
                .push(bill);
        }
    }

    let mut result = Vec::new();
    let mut current_date = start_date;
    while current_date <= end_date {
        let date_text = format_date(current_date);
        let opening_balances = current_balances.clone();

        for bill in bills_by_date.get(&date_text).into_iter().flatten() {
            apply_asset_trend_bill(&mut current_balances, bill);
        }

        let items = accounts
            .iter()
            .map(|account| AssetTrendAccountItem {
                account_id: account.id.to_string(),
                account_opening_balance_cents: *opening_balances.get(&account.id).unwrap_or(&0),
                account_closing_balance_cents: *current_balances.get(&account.id).unwrap_or(&0),
            })
            .collect();

        result.push(AssetTrendDay {
            year: current_date.year(),
            month: current_date.month(),
            day: current_date.day(),
            items,
        });
        current_date += Duration::days(1);
    }
    result
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn non_empty_asset_trend_account_ids(days: &[AssetTrendDay]) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for day in days {
        for item in &day.items {
            if item.account_opening_balance_cents != 0
                || item.account_closing_balance_cents != 0
                || item.account_opening_balance_cents != item.account_closing_balance_cents
            {
                ids.insert(item.account_id.clone());
            }
        }
    }
    ids
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_asset_trend_legend(
    accounts: &[StatisticsAccountInput],
    days: &[AssetTrendDay],
) -> Vec<AssetTrendLegendItem> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_asset_trend_legend",
        "business operation entered"
    );
    let non_empty_ids = non_empty_asset_trend_account_ids(days);
    accounts
        .iter()
        .filter(|account| non_empty_ids.contains(&account.id.to_string()))
        .map(|account| AssetTrendLegendItem {
            id: account.id.to_string(),
            name: account.name.clone(),
        })
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_net_worth_snapshot(accounts: &[StatisticsAccountInput]) -> NetWorthSnapshot {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_net_worth_snapshot",
        "business operation entered"
    );
    let mut assets = Vec::new();
    let mut liabilities = Vec::new();
    let mut total_assets_cents = 0_i64;
    let mut total_liabilities_cents = 0_i64;

    for account in accounts.iter().filter(|account| !account.hidden) {
        let balance_cents = account.balance_cents;
        let account_type = account.account_type.to_lowercase();
        let entry = NetWorthAccountEntry {
            id: account.id,
            name: account.name.clone(),
            account_type: account_type.clone(),
            icon: account.icon.clone(),
            balance_cents,
            currency: account
                .currency
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "CNY".to_string()),
        };

        if is_liability_account_type(&account_type) {
            total_liabilities_cents += balance_cents.abs();
            liabilities.push(entry);
        } else {
            total_assets_cents += balance_cents;
            assets.push(entry);
        }
    }

    NetWorthSnapshot {
        account_count: assets.len() + liabilities.len(),
        assets,
        liabilities,
        total_assets_cents,
        total_liabilities_cents,
        net_worth_cents: total_assets_cents - total_liabilities_cents,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_net_worth_snapshot_response(snapshot: &NetWorthSnapshot) -> Value {
    json!({"success": true, "data": snapshot})
}
