use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::Money;

pub const DEFAULT_EXCHANGE_RATE_PROVIDER_ORDER: [&str; 4] = ["boc_cn", "cmb_cn", "ecb", "rba"];
pub const TARGET_EXCHANGE_CURRENCIES: [&str; 16] = [
    "USD", "EUR", "GBP", "JPY", "HKD", "KRW", "AUD", "CAD", "SGD", "TWD", "MYR", "THB", "VND",
    "CHF", "NZD", "CNY",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatisticsContractError {
    pub error: String,
    pub message: String,
}

impl StatisticsContractError {
    pub fn new(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum StatisticsTimestampRange {
    All,
    Bounded { start_time: i64, end_time: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsYearMonthRange {
    pub start_year: i32,
    pub start_month: u32,
    pub end_year: i32,
    pub end_month: u32,
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum StatisticsYearMonthRangeMode {
    All,
    Bounded(StatisticsYearMonthRange),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatisticsBillInput {
    pub id: Option<i64>,
    pub date: String,
    pub bill_type: String,
    pub amount_yuan: String,
    pub channel: String,
    pub source_account_id: Option<i64>,
    pub destination_account_id: Option<i64>,
    pub destination_account: String,
    pub destination_amount_yuan: Option<String>,
    pub main_category: String,
    pub sub_category: String,
    pub counterparty: String,
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatisticsCategoryInput {
    pub id: i64,
    pub main_category: String,
    pub sub_category: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatisticsAccountInput {
    pub id: i64,
    pub name: String,
    pub account_type: String,
    pub hidden: bool,
    pub balance_yuan: String,
    pub initial_balance_yuan: String,
    pub currency: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryStatisticItem {
    pub category_id: String,
    pub account_id: String,
    pub amount: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryTrendBucket {
    pub year: i32,
    pub month: u32,
    pub items: Vec<CategoryStatisticItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTrendAccountItem {
    pub account_id: String,
    pub account_opening_balance: i64,
    pub account_closing_balance: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetTrendDay {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub items: Vec<AssetTrendAccountItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetTrendLegendItem {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetWorthAccountEntry {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub icon: Option<String>,
    pub balance: f64,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetWorthSnapshot {
    pub assets: Vec<NetWorthAccountEntry>,
    pub liabilities: Vec<NetWorthAccountEntry>,
    pub total_assets: f64,
    pub total_liabilities: f64,
    pub net_worth: f64,
    pub account_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecurringRuleInput {
    pub id: Option<i64>,
    pub name: String,
    pub amount_yuan: String,
    pub bill_type: String,
    pub frequency: String,
    pub next_date: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarBillItem {
    pub id: Option<i64>,
    pub amount: f64,
    #[serde(rename = "type")]
    pub bill_type: String,
    pub counterparty: String,
    pub description: String,
    pub main_category: String,
    pub sub_category: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventDay {
    pub date: String,
    pub income: f64,
    pub expense: f64,
    pub transfer_in: f64,
    pub transfer_out: f64,
    pub net: f64,
    pub count: usize,
    pub bills: Vec<CalendarBillItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarRecurringProjection {
    pub date: String,
    #[serde(rename = "type")]
    pub projection_type: String,
    pub name: String,
    pub amount: f64,
    pub bill_type: String,
    pub frequency: String,
    pub recurring_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventsData {
    pub events: Vec<CalendarEventDay>,
    pub recurring_projections: Vec<CalendarRecurringProjection>,
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeRateProviderOption {
    pub label: String,
    pub reference_url: String,
    pub region: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExchangeRateItem {
    pub currency: String,
    pub rate: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeRatesResult {
    pub provider_key: String,
    pub requested_provider: String,
    pub fallback_used: bool,
    pub data_source: String,
    pub reference_url: String,
    pub update_time: i64,
    pub base_currency: String,
    pub exchange_rates: Vec<ExchangeRateItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserCustomExchangeRateInput {
    pub to_currency: String,
    pub rate: String,
    pub effective_date: Option<String>,
    pub effective_timestamp: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NameValueStatisticItem {
    pub name: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopMerchantStatisticItem {
    pub name: String,
    pub amount: f64,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionAmountBucket {
    pub currency: String,
    pub income_amount: i64,
    pub expense_amount: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionAmountPeriodResult {
    pub start_time: i64,
    pub end_time: i64,
    pub amounts: Vec<TransactionAmountBucket>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatisticsTrendPoint {
    pub date: String,
    pub income: f64,
    pub expense: f64,
    pub net: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatisticsAnalyzerPeriodRange {
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatisticsAnalyzerTrendBucket {
    pub period: String,
    pub income: f64,
    pub expense: f64,
    pub net: f64,
}

pub fn parse_statistics_timestamp_range(
    start_raw: Option<&str>,
    end_raw: Option<&str>,
) -> Result<StatisticsTimestampRange, StatisticsContractError> {
    let start_text = start_raw.unwrap_or_default().trim();
    let end_text = end_raw.unwrap_or_default().trim();
    if start_text == "0" && end_text == "0" {
        return Ok(StatisticsTimestampRange::All);
    }

    let start_time = parse_i64_text(start_text, "Invalid timestamp format")?;
    let end_time = parse_i64_text(end_text, "Invalid timestamp format")?;
    validate_statistics_time_range(start_time, end_time)?;
    Ok(StatisticsTimestampRange::Bounded {
        start_time,
        end_time,
    })
}

pub fn validate_statistics_time_range(
    start_time: i64,
    end_time: i64,
) -> Result<(), StatisticsContractError> {
    if start_time > end_time {
        return Err(StatisticsContractError::new(
            "Invalid time range",
            "startTime must be less than or equal to endTime",
        ));
    }
    Ok(())
}

pub fn validate_asset_trends_span(
    start_time: i64,
    end_time: i64,
    is_all_mode: bool,
) -> Result<(), StatisticsContractError> {
    validate_statistics_time_range(start_time, end_time)?;
    let day_count = (end_time - start_time) / 86_400;
    if !is_all_mode && day_count > 365 {
        let message = "资产趋势查询最多支持365天范围，请缩小时间范围";
        return Err(StatisticsContractError::new(message, message));
    }
    Ok(())
}

pub fn parse_statistics_year_month_range(
    start_raw: Option<&str>,
    end_raw: Option<&str>,
) -> Result<StatisticsYearMonthRangeMode, StatisticsContractError> {
    let start_clean = clean_year_month_text(start_raw.unwrap_or_default());
    let end_clean = clean_year_month_text(end_raw.unwrap_or_default());
    if matches!(start_clean.as_str(), "0" | "197001")
        && matches!(end_clean.as_str(), "0" | "197001")
    {
        return Ok(StatisticsYearMonthRangeMode::All);
    }

    let (start_year, start_month) = parse_year_month_clean(&start_clean)?;
    let (end_year, end_month) = parse_year_month_clean(&end_clean)?;
    if (start_year, start_month) > (end_year, end_month) {
        return Err(StatisticsContractError::new(
            "Invalid year-month range",
            "startYearMonth must be less than or equal to endYearMonth",
        ));
    }

    let end_date = last_day_of_month(end_year, end_month)?;
    Ok(StatisticsYearMonthRangeMode::Bounded(
        StatisticsYearMonthRange {
            start_year,
            start_month,
            end_year,
            end_month,
            start_date: format!("{start_year:04}-{start_month:02}-01"),
            end_date: format_date(end_date),
        },
    ))
}

pub fn build_category_statistics_items(
    bills: &[StatisticsBillInput],
    categories: &[StatisticsCategoryInput],
    accounts: &[StatisticsAccountInput],
) -> Vec<CategoryStatisticItem> {
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
                amount,
            },
        )
        .collect()
}

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

pub fn build_category_trend_statistics(
    bills: &[StatisticsBillInput],
    categories: &[StatisticsCategoryInput],
    accounts: &[StatisticsAccountInput],
    range: &StatisticsYearMonthRange,
) -> Vec<CategoryTrendBucket> {
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
                    amount,
                },
            )
            .collect();
        CategoryTrendBucket { year, month, items }
    })
    .collect()
}

pub fn build_asset_trends(
    bills: &[StatisticsBillInput],
    accounts: &[StatisticsAccountInput],
    balances_before_date_yuan: &BTreeMap<i64, String>,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Vec<AssetTrendDay> {
    let mut current_balances: BTreeMap<i64, i64> = BTreeMap::new();
    for account in accounts {
        let initial = yuan_to_cents_lossy(&account.initial_balance_yuan);
        let history = balances_before_date_yuan
            .get(&account.id)
            .map(|value| yuan_to_cents_lossy(value))
            .unwrap_or(0);
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
                account_opening_balance: *opening_balances.get(&account.id).unwrap_or(&0),
                account_closing_balance: *current_balances.get(&account.id).unwrap_or(&0),
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

pub fn non_empty_asset_trend_account_ids(days: &[AssetTrendDay]) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for day in days {
        for item in &day.items {
            if item.account_opening_balance != 0
                || item.account_closing_balance != 0
                || item.account_opening_balance != item.account_closing_balance
            {
                ids.insert(item.account_id.clone());
            }
        }
    }
    ids
}

pub fn build_asset_trend_legend(
    accounts: &[StatisticsAccountInput],
    days: &[AssetTrendDay],
) -> Vec<AssetTrendLegendItem> {
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

pub fn build_net_worth_snapshot(accounts: &[StatisticsAccountInput]) -> NetWorthSnapshot {
    let mut assets = Vec::new();
    let mut liabilities = Vec::new();
    let mut total_assets_cents = 0_i64;
    let mut total_liabilities_cents = 0_i64;

    for account in accounts.iter().filter(|account| !account.hidden) {
        let balance_cents = yuan_to_cents_lossy(&account.balance_yuan);
        let account_type = account.account_type.to_lowercase();
        let entry = NetWorthAccountEntry {
            id: account.id,
            name: account.name.clone(),
            account_type: account_type.clone(),
            icon: account.icon.clone(),
            balance: cents_to_yuan(balance_cents),
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
        total_assets: cents_to_yuan(total_assets_cents),
        total_liabilities: cents_to_yuan(total_liabilities_cents),
        net_worth: cents_to_yuan(total_assets_cents - total_liabilities_cents),
    }
}

pub fn build_net_worth_snapshot_response(snapshot: &NetWorthSnapshot) -> Value {
    json!({"success": true, "data": snapshot})
}

pub fn build_insight_anomaly_summary(
    bills: &[StatisticsBillInput],
    analyzed_months: u32,
    start_date: &str,
    end_date: &str,
) -> Value {
    let mut anomalies = Vec::<Value>::new();
    let mut category_amounts: BTreeMap<String, Vec<i64>> = BTreeMap::new();

    for bill in bills {
        if is_expense_type(&bill.bill_type) {
            let amount = yuan_to_cents_lossy(&bill.amount_yuan).abs();
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
        let amount = yuan_to_cents_lossy(&bill.amount_yuan).abs();
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
                "amount": cents_to_yuan(amount),
                "category": category,
                "average": cents_to_yuan(avg.round() as i64),
                "ratio": round_one_decimal((amount as f64) / avg),
                "description": first_non_empty(&bill.description, &bill.counterparty),
                "message": format!(
                    "此笔交易金额 ¥{:.2} 是 {} 分类平均值 ¥{:.2} 的 {:.1} 倍",
                    cents_to_yuan(amount),
                    category,
                    cents_to_yuan(avg.round() as i64),
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

pub fn build_calendar_events_data(
    bills: &[StatisticsBillInput],
    recurring_rules: &[RecurringRuleInput],
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> CalendarEventsData {
    let mut daily: BTreeMap<String, CalendarEventDay> = BTreeMap::new();
    for bill in bills {
        let Some(date) = parse_bill_date_prefix(&bill.date) else {
            continue;
        };
        let date_text = format_date(date);
        let day = daily
            .entry(date_text.clone())
            .or_insert_with(|| empty_calendar_day(date_text.clone()));
        day.count += 1;

        let amount_cents = yuan_to_cents_lossy(&bill.amount_yuan).abs();
        if is_expense_type(&bill.bill_type) {
            day.expense = round_money(day.expense + cents_to_yuan(amount_cents));
        } else if is_income_type(&bill.bill_type) {
            day.income = round_money(day.income + cents_to_yuan(amount_cents));
        } else if is_transfer_type(&bill.bill_type) {
            day.transfer_out = round_money(day.transfer_out + cents_to_yuan(amount_cents));
        }
        day.net = round_money(day.income - day.expense);
        day.bills.push(CalendarBillItem {
            id: bill.id,
            amount: cents_to_yuan(yuan_to_cents_lossy(&bill.amount_yuan)),
            bill_type: bill.bill_type.clone(),
            counterparty: bill.counterparty.clone(),
            description: bill.description.clone(),
            main_category: bill.main_category.clone(),
            sub_category: bill.sub_category.clone(),
        });
    }

    let recurring_projections =
        build_calendar_recurring_projections(recurring_rules, start_date, end_date);

    CalendarEventsData {
        events: daily.into_values().collect(),
        recurring_projections,
        start_date: format_date(start_date),
        end_date: format_date(end_date),
    }
}

pub fn build_calendar_events_response(data: &CalendarEventsData) -> Value {
    json!({"success": true, "data": data})
}

pub fn build_category_pie_data(bills: &[StatisticsBillInput]) -> Vec<NameValueStatisticItem> {
    let mut totals: BTreeMap<String, i64> = BTreeMap::new();
    for bill in bills {
        let category = bill.main_category.clone();
        *totals.entry(category).or_insert(0) += yuan_to_cents_lossy(&bill.amount_yuan).abs();
    }

    let mut result = totals
        .into_iter()
        .map(|(name, total)| NameValueStatisticItem {
            name,
            value: cents_to_yuan(total),
        })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| {
        right
            .value
            .partial_cmp(&left.value)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.name.cmp(&right.name))
    });
    result
}

pub fn build_top_merchants_data(
    bills: &[StatisticsBillInput],
    limit: usize,
) -> Vec<TopMerchantStatisticItem> {
    let mut totals: BTreeMap<String, (i64, usize)> = BTreeMap::new();
    for bill in bills {
        let merchant = if bill.counterparty.trim().is_empty() {
            "未知商家".to_string()
        } else {
            bill.counterparty.clone()
        };
        let entry = totals.entry(merchant).or_insert((0, 0));
        entry.0 += yuan_to_cents_lossy(&bill.amount_yuan).abs();
        entry.1 += 1;
    }

    let mut result = totals
        .into_iter()
        .map(|(name, (amount, count))| TopMerchantStatisticItem {
            name,
            amount: cents_to_yuan(amount),
            count,
        })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| {
        right
            .amount
            .partial_cmp(&left.amount)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.name.cmp(&right.name))
    });
    result.truncate(limit);
    result
}

pub fn parse_transaction_amount_period_query(period_query: &str) -> Option<(String, i64, i64)> {
    let parts = period_query.split('_').collect::<Vec<_>>();
    if parts.len() != 3 {
        return None;
    }
    let start_time = parts[1].parse::<i64>().ok()?;
    let end_time = parts[2].parse::<i64>().ok()?;
    Some((parts[0].to_string(), start_time, end_time))
}

pub fn build_transaction_amount_period_result(
    start_time: i64,
    end_time: i64,
    bills: &[StatisticsBillInput],
) -> TransactionAmountPeriodResult {
    let total_income = bills
        .iter()
        .filter(|bill| is_income_type(&bill.bill_type))
        .map(|bill| yuan_to_cents_lossy(&bill.amount_yuan).abs())
        .sum::<i64>();
    let total_expense = bills
        .iter()
        .filter(|bill| is_expense_type(&bill.bill_type))
        .map(|bill| yuan_to_cents_lossy(&bill.amount_yuan).abs())
        .sum::<i64>();

    TransactionAmountPeriodResult {
        start_time,
        end_time,
        amounts: vec![TransactionAmountBucket {
            currency: "CNY".to_string(),
            income_amount: total_income,
            expense_amount: total_expense,
        }],
    }
}

pub fn build_transaction_amounts_response(
    period_results: &BTreeMap<String, TransactionAmountPeriodResult>,
) -> Value {
    json!({"success": true, "result": period_results})
}

pub fn build_statistics_trend_points(analyzer_result: &Value) -> Vec<StatisticsTrendPoint> {
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
            income: round_money(trend.get("income").and_then(Value::as_f64).unwrap_or(0.0)),
            expense: round_money(trend.get("expense").and_then(Value::as_f64).unwrap_or(0.0)),
            net: round_money(trend.get("net").and_then(Value::as_f64).unwrap_or(0.0)),
        })
        .collect()
}

pub fn build_statistics_trend_response(analyzer_result: &Value) -> Value {
    json!({"success": true, "data": build_statistics_trend_points(analyzer_result)})
}

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

pub fn build_statistics_analyzer_report(
    period: &str,
    range: &StatisticsAnalyzerPeriodRange,
    bills: &[StatisticsBillInput],
    generated_at: &str,
) -> Value {
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

pub fn build_statistics_analyzer_trends_result(
    period: &str,
    category: Option<&str>,
    buckets: &[StatisticsAnalyzerTrendBucket],
) -> Value {
    json!({
        "trends": buckets,
        "period": period,
        "category": category,
    })
}

pub fn build_statistics_analyzer_trend_bucket(
    period: &str,
    bills: &[StatisticsBillInput],
) -> StatisticsAnalyzerTrendBucket {
    let income_cents = bills
        .iter()
        .filter(|bill| is_income_type(&bill.bill_type))
        .map(|bill| yuan_to_cents_lossy(&bill.amount_yuan))
        .sum::<i64>();
    let expense_cents = bills
        .iter()
        .filter(|bill| is_expense_type(&bill.bill_type))
        .map(|bill| yuan_to_cents_lossy(&bill.amount_yuan))
        .sum::<i64>();
    let income = round_money(cents_to_yuan(income_cents));
    let expense = round_money(cents_to_yuan(expense_cents));
    StatisticsAnalyzerTrendBucket {
        period: period.to_string(),
        income,
        expense,
        net: round_money(income - expense),
    }
}

pub fn build_statistics_analyzer_comparison_result(
    period: &str,
    compare_type: &str,
    bills: &[StatisticsBillInput],
) -> Value {
    let comparison = if compare_type == "category" {
        let mut categories: BTreeMap<String, (i64, i64, usize)> = BTreeMap::new();
        for bill in bills {
            let entry = categories
                .entry(main_category_or_uncategorized(bill))
                .or_insert((0, 0, 0));
            if is_income_type(&bill.bill_type) {
                entry.0 += yuan_to_cents_lossy(&bill.amount_yuan);
            } else if is_expense_type(&bill.bill_type) {
                entry.1 += yuan_to_cents_lossy(&bill.amount_yuan);
            }
            entry.2 += 1;
        }
        let mut rows = categories
            .into_iter()
            .map(|(name, (income_cents, expense_cents, count))| {
                let income = round_money(cents_to_yuan(income_cents));
                let expense = round_money(cents_to_yuan(expense_cents));
                json!({
                    "name": name,
                    "income": income,
                    "expense": expense,
                    "count": count,
                    "net": round_money(income - expense),
                })
            })
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| {
            let left_expense = left.get("expense").and_then(Value::as_f64).unwrap_or(0.0);
            let right_expense = right.get("expense").and_then(Value::as_f64).unwrap_or(0.0);
            right_expense
                .partial_cmp(&left_expense)
                .unwrap_or(std::cmp::Ordering::Equal)
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

pub fn build_statistics_analyzer_category_result(
    period: &str,
    main_category: Option<&str>,
    bills: &[StatisticsBillInput],
) -> Value {
    let mut sub_categories: BTreeMap<String, (i64, usize)> = BTreeMap::new();
    let mut total_cents = 0_i64;
    for bill in bills.iter().filter(|bill| is_expense_type(&bill.bill_type)) {
        let amount = yuan_to_cents_lossy(&bill.amount_yuan);
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
            let amount = round_money(cents_to_yuan(amount_cents));
            let percentage = if total_cents > 0 {
                round_money((amount_cents as f64 / total_cents as f64) * 100.0)
            } else {
                0.0
            };
            json!({
                "sub_category": sub_category,
                "amount": amount,
                "count": count,
                "percentage": percentage,
                "avg_amount": if count > 0 { round_money(amount / count as f64) } else { 0.0 },
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        let left_amount = left.get("amount").and_then(Value::as_f64).unwrap_or(0.0);
        let right_amount = right.get("amount").and_then(Value::as_f64).unwrap_or(0.0);
        right_amount
            .partial_cmp(&left_amount)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    json!({
        "main_category": main_category,
        "period": period,
        "total_amount": round_money(cents_to_yuan(total_cents)),
        "sub_categories": rows,
    })
}

pub fn exchange_rate_provider_options() -> BTreeMap<String, ExchangeRateProviderOption> {
    BTreeMap::from([
        (
            "auto".to_string(),
            ExchangeRateProviderOption {
                label: "自动选择".to_string(),
                reference_url: String::new(),
                region: "mixed".to_string(),
            },
        ),
        (
            "boc_cn".to_string(),
            ExchangeRateProviderOption {
                label: "中国银行外汇牌价".to_string(),
                reference_url: "https://www.boc.cn/sourcedb/whpj/".to_string(),
                region: "domestic".to_string(),
            },
        ),
        (
            "cmb_cn".to_string(),
            ExchangeRateProviderOption {
                label: "招商银行实时汇率".to_string(),
                reference_url: "https://fx.cmbchina.com/hq/".to_string(),
                region: "domestic".to_string(),
            },
        ),
        (
            "ecb".to_string(),
            ExchangeRateProviderOption {
                label: "ECB (欧洲央行)".to_string(),
                reference_url: "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml"
                    .to_string(),
                region: "foreign".to_string(),
            },
        ),
        (
            "rba".to_string(),
            ExchangeRateProviderOption {
                label: "RBA (澳大利亚储备银行)".to_string(),
                reference_url: "https://www.rba.gov.au/rss/rss-cb-exchange-rates.xml".to_string(),
                region: "foreign".to_string(),
            },
        ),
    ])
}

pub fn normalize_requested_exchange_rate_provider(raw: Option<&str>) -> String {
    let provider = raw.unwrap_or("auto").trim().to_lowercase();
    if provider.is_empty() {
        "auto".to_string()
    } else {
        provider
    }
}

pub fn build_provider_candidate_order(requested_provider: &str) -> Vec<String> {
    if requested_provider == "auto" {
        return DEFAULT_EXCHANGE_RATE_PROVIDER_ORDER
            .iter()
            .map(|provider| (*provider).to_string())
            .collect();
    }

    if !exchange_rate_provider_options().contains_key(requested_provider) {
        return Vec::new();
    }

    let mut order = vec![requested_provider.to_string()];
    for provider in DEFAULT_EXCHANGE_RATE_PROVIDER_ORDER {
        if provider != requested_provider {
            order.push(provider.to_string());
        }
    }
    order
}

pub fn build_user_custom_exchange_rates_result(
    base_currency: &str,
    custom_rates: &[UserCustomExchangeRateInput],
    now_timestamp: i64,
) -> ExchangeRatesResult {
    let mut exchange_rates = vec![ExchangeRateItem {
        currency: base_currency.to_string(),
        rate: "1.0".to_string(),
    }];
    let mut update_time = now_timestamp;
    for rate in custom_rates {
        exchange_rates.push(ExchangeRateItem {
            currency: rate.to_currency.clone(),
            rate: rate.rate.clone(),
        });
        if let Some(effective) = rate
            .effective_timestamp
            .or_else(|| parse_effective_date_timestamp(rate.effective_date.as_deref()))
        {
            update_time = update_time.max(effective);
        }
    }

    ExchangeRatesResult {
        provider_key: "user_custom".to_string(),
        requested_provider: "auto".to_string(),
        fallback_used: false,
        data_source: "user_custom".to_string(),
        reference_url: String::new(),
        update_time,
        base_currency: base_currency.to_string(),
        exchange_rates,
    }
}

pub fn build_provider_exchange_rates_result(
    base_currency: &str,
    requested_provider: &str,
    provider_key: &str,
    rates: &BTreeMap<String, f64>,
    update_time: i64,
) -> ExchangeRatesResult {
    let option = exchange_rate_provider_options()
        .get(provider_key)
        .cloned()
        .unwrap_or_else(|| ExchangeRateProviderOption {
            label: provider_key.to_string(),
            reference_url: String::new(),
            region: String::new(),
        });
    let mut exchange_rates = vec![ExchangeRateItem {
        currency: base_currency.to_string(),
        rate: "1.0".to_string(),
    }];
    for (currency, rate) in rates {
        exchange_rates.push(ExchangeRateItem {
            currency: currency.clone(),
            rate: format_python_round_rate(*rate),
        });
    }

    ExchangeRatesResult {
        provider_key: provider_key.to_string(),
        requested_provider: requested_provider.to_string(),
        fallback_used: requested_provider != "auto" && requested_provider != provider_key,
        data_source: option.label,
        reference_url: option.reference_url,
        update_time,
        base_currency: base_currency.to_string(),
        exchange_rates,
    }
}

pub fn build_builtin_fallback_exchange_rates(
    base_currency: &str,
    update_time: i64,
) -> ExchangeRatesResult {
    let cny_rates = builtin_cny_based_rates();
    let mut exchange_rates = Vec::new();
    if base_currency == "CNY" {
        exchange_rates.push(ExchangeRateItem {
            currency: "CNY".to_string(),
            rate: "1.0".to_string(),
        });
        for (currency, rate) in cny_rates {
            exchange_rates.push(ExchangeRateItem {
                currency: currency.to_string(),
                rate: format_python_round_rate(rate),
            });
        }
    } else if let Some(base_rate) = builtin_cny_based_rates()
        .into_iter()
        .find_map(|(currency, rate)| (currency == base_currency).then_some(rate))
    {
        exchange_rates.push(ExchangeRateItem {
            currency: base_currency.to_string(),
            rate: "1.0".to_string(),
        });
        exchange_rates.push(ExchangeRateItem {
            currency: "CNY".to_string(),
            rate: format_python_round_rate(1.0 / base_rate),
        });
        for (currency, rate) in builtin_cny_based_rates() {
            if currency != base_currency {
                exchange_rates.push(ExchangeRateItem {
                    currency: currency.to_string(),
                    rate: format_python_round_rate(rate / base_rate),
                });
            }
        }
    }

    ExchangeRatesResult {
        provider_key: "fallback".to_string(),
        requested_provider: "auto".to_string(),
        fallback_used: true,
        data_source: "内置汇率数据 (回退)".to_string(),
        reference_url: String::new(),
        update_time,
        base_currency: base_currency.to_string(),
        exchange_rates,
    }
}

pub fn convert_cny_quote_map_to_rates(
    quote_map: &BTreeMap<String, f64>,
    base_currency: &str,
    target_currencies: &[String],
) -> BTreeMap<String, f64> {
    let mut result = BTreeMap::new();
    if base_currency == "CNY" {
        for currency in target_currencies {
            if let Some(quote) = quote_map
                .get(currency)
                .copied()
                .filter(|quote| *quote > 0.0)
            {
                result.insert(currency.clone(), 100.0 / quote);
            }
        }
        return result;
    }

    let Some(base_quote) = quote_map
        .get(base_currency)
        .copied()
        .filter(|quote| *quote > 0.0)
    else {
        return result;
    };

    for currency in target_currencies {
        if currency == "CNY" {
            result.insert(currency.clone(), base_quote / 100.0);
        } else if let Some(target_quote) = quote_map
            .get(currency)
            .copied()
            .filter(|quote| *quote > 0.0)
        {
            result.insert(currency.clone(), base_quote / target_quote);
        }
    }
    result
}

pub fn convert_provider_base_currency(
    rates: &BTreeMap<String, f64>,
    original_base: &str,
    target_base: &str,
    target_currencies: &[String],
    rate_format: &str,
) -> BTreeMap<String, f64> {
    if original_base == target_base {
        return target_currencies
            .iter()
            .filter_map(|currency| rates.get(currency).map(|rate| (currency.clone(), *rate)))
            .collect();
    }

    let Some(base_rate) = rates.get(target_base).copied().filter(|rate| *rate != 0.0) else {
        return BTreeMap::new();
    };

    let mut result = BTreeMap::new();
    for currency in target_currencies {
        if currency == target_base {
            result.insert(currency.clone(), 1.0);
        } else if currency == original_base {
            if rate_format == "target_to_base" {
                result.insert(currency.clone(), base_rate);
            } else {
                result.insert(currency.clone(), 1.0 / base_rate);
            }
        } else if let Some(rate) = rates.get(currency).copied() {
            if rate_format == "target_to_base" {
                result.insert(currency.clone(), base_rate / rate);
            } else {
                result.insert(currency.clone(), rate / base_rate);
            }
        }
    }
    result
}

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

pub fn build_overview_result_from_report(report: &Value) -> Value {
    let summary = report.get("summary").unwrap_or(&Value::Null);
    let total_income = round_money(
        summary
            .get("total_income")
            .and_then(Value::as_f64)
            .unwrap_or(0.0),
    );
    let total_expense = round_money(
        summary
            .get("total_expense")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            .abs(),
    );
    json!({
        "total_income": total_income,
        "total_expense": total_expense,
        "net_income": round_money(total_income - total_expense),
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

pub fn build_statistics_report_chart_plan(report: &Value) -> Vec<String> {
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

fn empty_statistics_analyzer_report(generated_at: &str) -> Value {
    json!({
        "period": "",
        "start_date": "",
        "end_date": "",
        "total_records": 0,
        "summary": {"total_income": 0, "total_expense": 0, "net_income": 0},
        "by_category": {},
        "by_type": {},
        "trend": [],
        "top_expenses": [],
        "top_income": [],
        "generated_at": generated_at,
    })
}

fn statistics_analyzer_summary(bills: &[StatisticsBillInput]) -> Value {
    let total_income = cents_to_yuan(
        bills
            .iter()
            .filter(|bill| is_income_type(&bill.bill_type))
            .map(|bill| yuan_to_cents_lossy(&bill.amount_yuan))
            .sum(),
    );
    let total_expense = cents_to_yuan(
        bills
            .iter()
            .filter(|bill| is_expense_type(&bill.bill_type))
            .map(|bill| yuan_to_cents_lossy(&bill.amount_yuan))
            .sum(),
    );
    json!({
        "total_income": total_income,
        "total_expense": total_expense,
        "net_income": total_income - total_expense,
    })
}

type AnalyzerSubCategoryTotals = BTreeMap<String, (usize, i64)>;
type AnalyzerCategoryTotals = BTreeMap<String, (usize, i64, AnalyzerSubCategoryTotals)>;

fn statistics_analyzer_by_category(bills: &[StatisticsBillInput]) -> Value {
    let mut categories: AnalyzerCategoryTotals = BTreeMap::new();
    for bill in bills {
        let amount = yuan_to_cents_lossy(&bill.amount_yuan);
        let entry = categories
            .entry(bill.main_category.clone())
            .or_insert((0, 0, BTreeMap::new()));
        entry.0 += 1;
        entry.1 += amount;
        let sub_entry = entry.2.entry(bill.sub_category.clone()).or_insert((0, 0));
        sub_entry.0 += 1;
        sub_entry.1 += amount;
    }

    let mut result = serde_json::Map::new();
    for (category, (count, total_cents, sub_categories)) in categories {
        let mut sub_result = serde_json::Map::new();
        for (sub_category, (sub_count, sub_total_cents)) in sub_categories {
            sub_result.insert(
                sub_category,
                json!({
                    "count": sub_count,
                    "total": cents_to_yuan(sub_total_cents),
                }),
            );
        }
        result.insert(
            category,
            json!({
                "count": count,
                "total": cents_to_yuan(total_cents),
                "average": if count > 0 { cents_to_yuan(total_cents) / count as f64 } else { 0.0 },
                "sub_categories": sub_result,
            }),
        );
    }
    Value::Object(result)
}

fn statistics_analyzer_by_type(bills: &[StatisticsBillInput]) -> Value {
    let mut by_type: BTreeMap<String, (usize, i64)> = BTreeMap::new();
    for bill in bills {
        let entry = by_type.entry(bill.bill_type.clone()).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += yuan_to_cents_lossy(&bill.amount_yuan);
    }
    let mut result = serde_json::Map::new();
    for (bill_type, (count, total_cents)) in by_type {
        result.insert(
            bill_type,
            json!({
                "count": count,
                "total": cents_to_yuan(total_cents),
                "average": if count > 0 { cents_to_yuan(total_cents) / count as f64 } else { 0.0 },
            }),
        );
    }
    Value::Object(result)
}

fn statistics_analyzer_report_trend(bills: &[StatisticsBillInput], period: &str) -> Vec<Value> {
    let mut buckets: BTreeMap<String, (i64, i64)> = BTreeMap::new();
    for bill in bills {
        let Some(date) = parse_bill_date_prefix(&bill.date) else {
            continue;
        };
        let bucket_date = match period {
            "quarter" => {
                let days_until_sunday = 6_i64 - i64::from(date.weekday().num_days_from_monday());
                date + Duration::days(days_until_sunday)
            }
            "year" => last_day_of_month(date.year(), date.month()).unwrap_or(date),
            _ => date,
        };
        let entry = buckets.entry(format_date(bucket_date)).or_insert((0, 0));
        if is_income_type(&bill.bill_type) {
            entry.0 += yuan_to_cents_lossy(&bill.amount_yuan);
        } else if is_expense_type(&bill.bill_type) {
            entry.1 += yuan_to_cents_lossy(&bill.amount_yuan);
        }
    }
    buckets
        .into_iter()
        .map(|(date, (income_cents, expense_cents))| {
            let income = cents_to_yuan(income_cents);
            let expense = cents_to_yuan(expense_cents);
            json!({
                "date": date,
                "income": income,
                "expense": expense,
                "net": income - expense,
            })
        })
        .collect()
}

fn statistics_analyzer_top_bills(
    bills: &[StatisticsBillInput],
    bill_type: &str,
    limit: usize,
    include_category: bool,
) -> Vec<Value> {
    let mut rows = bills
        .iter()
        .filter(|bill| bill.bill_type == bill_type)
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        yuan_to_cents_lossy(&right.amount_yuan).cmp(&yuan_to_cents_lossy(&left.amount_yuan))
    });
    rows.into_iter()
        .take(limit)
        .map(|bill| {
            let mut item = serde_json::Map::from_iter([
                (
                    "date".to_string(),
                    json!(bill.date.chars().take(10).collect::<String>()),
                ),
                (
                    "amount".to_string(),
                    json!(cents_to_yuan(yuan_to_cents_lossy(&bill.amount_yuan))),
                ),
                ("counterparty".to_string(), json!(bill.counterparty)),
                ("description".to_string(), json!(bill.description)),
            ]);
            if include_category {
                item.insert("category".to_string(), json!(bill.main_category));
            }
            Value::Object(item)
        })
        .collect()
}

fn first_day(year: i32, month: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, 1).expect("valid first day")
}

fn add_months(date: NaiveDate, months: u32) -> NaiveDate {
    let zero_based = date.month0() + months;
    let year = date.year() + (zero_based / 12) as i32;
    let month = (zero_based % 12) + 1;
    first_day(year, month)
}

fn parse_i64_text(text: &str, error: &str) -> Result<i64, StatisticsContractError> {
    text.parse::<i64>()
        .map_err(|exc| StatisticsContractError::new(error, format!("{error}: {exc}")))
}

fn clean_year_month_text(text: &str) -> String {
    text.trim().replace('-', "")
}

fn parse_year_month_clean(text: &str) -> Result<(i32, u32), StatisticsContractError> {
    if text.len() < 6 {
        return Err(StatisticsContractError::new(
            "Invalid year-month format",
            "expected: 202411 or 2024-11",
        ));
    }
    let year = text[0..4].parse::<i32>().map_err(|exc| {
        StatisticsContractError::new(
            "Invalid year-month format",
            format!("Invalid year-month format (expected: 202411 or 2024-11): {exc}"),
        )
    })?;
    let month = text[4..6].parse::<u32>().map_err(|exc| {
        StatisticsContractError::new(
            "Invalid year-month format",
            format!("Invalid year-month format (expected: 202411 or 2024-11): {exc}"),
        )
    })?;
    if !(1..=12).contains(&month) {
        return Err(StatisticsContractError::new(
            "Invalid year-month format",
            "Invalid year-month format (expected month 1..12)",
        ));
    }
    Ok((year, month))
}

fn last_day_of_month(year: i32, month: u32) -> Result<NaiveDate, StatisticsContractError> {
    let next_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .ok_or_else(|| StatisticsContractError::new("Invalid year-month format", "invalid date"))?;
    Ok(next_month - Duration::days(1))
}

fn iter_year_months(
    start_year: i32,
    start_month: u32,
    end_year: i32,
    end_month: u32,
) -> Vec<(i32, u32)> {
    let mut result = Vec::new();
    let (mut year, mut month) = (start_year, start_month);
    while (year, month) <= (end_year, end_month) {
        result.push((year, month));
        month += 1;
        if month > 12 {
            month = 1;
            year += 1;
        }
    }
    result
}

fn build_category_name_to_id(categories: &[StatisticsCategoryInput]) -> BTreeMap<String, String> {
    categories
        .iter()
        .map(|category| {
            (
                category_key(&category.main_category, &category.sub_category),
                category.id.to_string(),
            )
        })
        .collect()
}

fn build_account_name_to_id(accounts: &[StatisticsAccountInput]) -> BTreeMap<String, String> {
    accounts
        .iter()
        .map(|account| (account.name.clone(), account.id.to_string()))
        .collect()
}

fn category_key(main_category: &str, sub_category: &str) -> String {
    if sub_category.is_empty() {
        main_category.to_string()
    } else {
        format!("{main_category}-{sub_category}")
    }
}

fn resolve_statistics_account_id(
    bill: &StatisticsBillInput,
    valid_account_ids: &BTreeSet<String>,
    account_name_to_id: &BTreeMap<String, String>,
) -> String {
    let mut account_id = bill.source_account_id.unwrap_or(0).to_string();
    if !valid_account_ids.contains(&account_id) {
        account_id = account_name_to_id
            .get(&bill.channel)
            .cloned()
            .unwrap_or_else(|| "0".to_string());
    }
    account_id
}

fn signed_statistics_amount_cents(bill: &StatisticsBillInput) -> i64 {
    let mut amount = yuan_to_cents_lossy(&bill.amount_yuan);
    if is_expense_type(&bill.bill_type) {
        amount = -amount.abs();
    } else if is_income_type(&bill.bill_type) {
        amount = amount.abs();
    } else if is_transfer_type(&bill.bill_type) {
        if !bill.destination_account.is_empty() && bill.destination_account != bill.channel {
            amount = -amount.abs();
        } else {
            amount = amount.abs();
        }
    }
    amount
}

fn apply_asset_trend_bill(current_balances: &mut BTreeMap<i64, i64>, bill: &StatisticsBillInput) {
    let amount = yuan_to_cents_lossy(&bill.amount_yuan).abs();
    if is_income_type(&bill.bill_type) {
        if let Some(source_id) = bill.source_account_id {
            if let Some(balance) = current_balances.get_mut(&source_id) {
                *balance += amount;
            }
        }
    } else if is_expense_type(&bill.bill_type) {
        if let Some(source_id) = bill.source_account_id {
            if let Some(balance) = current_balances.get_mut(&source_id) {
                *balance -= amount;
            }
        }
    } else if is_transfer_type(&bill.bill_type) {
        if let Some(source_id) = bill.source_account_id {
            if let Some(balance) = current_balances.get_mut(&source_id) {
                *balance -= amount;
            }
        }
        if let Some(destination_id) = bill.destination_account_id {
            if let Some(balance) = current_balances.get_mut(&destination_id) {
                let destination_amount = bill
                    .destination_amount_yuan
                    .as_deref()
                    .map(yuan_to_cents_lossy)
                    .filter(|amount| *amount != 0)
                    .unwrap_or(amount)
                    .abs();
                *balance += destination_amount;
            }
        }
    }
}

fn parse_bill_date_prefix(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.get(0..10)?, "%Y-%m-%d").ok()
}

fn parse_effective_date_timestamp(value: Option<&str>) -> Option<i64> {
    let text = value?.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(timestamp) = text.parse::<i64>() {
        return Some(timestamp);
    }
    if let Ok(datetime) = chrono::DateTime::parse_from_rfc3339(text) {
        return Some(datetime.timestamp());
    }
    if let Ok(datetime) = chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S") {
        return Some(datetime.and_utc().timestamp());
    }
    NaiveDate::parse_from_str(text.get(0..10)?, "%Y-%m-%d")
        .ok()
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|datetime| datetime.and_utc().timestamp())
}

fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn yuan_to_cents_lossy(raw_value: &str) -> i64 {
    Money::from_yuan_str(raw_value.trim())
        .map(Money::to_cents)
        .unwrap_or(0)
}

fn cents_to_yuan(cents: i64) -> f64 {
    round_money((cents as f64) / 100.0)
}

fn round_money(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn round_one_decimal(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn is_expense_type(value: &str) -> bool {
    matches!(value.trim().to_lowercase().as_str(), "expense" | "支出")
}

fn is_income_type(value: &str) -> bool {
    matches!(value.trim().to_lowercase().as_str(), "income" | "收入")
}

fn is_transfer_type(value: &str) -> bool {
    matches!(value.trim().to_lowercase().as_str(), "transfer" | "转账")
}

fn is_liability_account_type(value: &str) -> bool {
    matches!(value, "credit_card" | "loan" | "debt" | "信用卡" | "贷款")
}

fn main_category_or_uncategorized(bill: &StatisticsBillInput) -> String {
    if bill.main_category.trim().is_empty() {
        "未分类".to_string()
    } else {
        bill.main_category.clone()
    }
}

fn average_cents(amounts: &[i64]) -> f64 {
    if amounts.is_empty() {
        0.0
    } else {
        amounts.iter().sum::<i64>() as f64 / amounts.len() as f64
    }
}

fn first_non_empty(first: &str, second: &str) -> String {
    if first.trim().is_empty() {
        second.to_string()
    } else {
        first.to_string()
    }
}

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
            let amount_a = yuan_to_cents_lossy(&bill_a.amount_yuan).abs();
            let amount_b = yuan_to_cents_lossy(&bill_b.amount_yuan).abs();
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
                        "amount": cents_to_yuan(amount_a),
                        "counterparty": bill_a.counterparty,
                        "message": format!(
                            "疑似重复扣款: {} ¥{:.2} ({} & {})",
                            bill_a.counterparty,
                            cents_to_yuan(amount_a),
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
            .or_insert(0) += yuan_to_cents_lossy(&bill.amount_yuan).abs();
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
                "currentAmount": cents_to_yuan(*latest_total),
                "averageAmount": cents_to_yuan(avg_prev.round() as i64),
                "ratio": round_one_decimal((*latest_total as f64) / avg_prev),
                "message": format!(
                    "{} 的 {} 支出 ¥{:.2} 是历史平均 ¥{:.2} 的 {:.1} 倍",
                    latest,
                    category,
                    cents_to_yuan(*latest_total),
                    cents_to_yuan(avg_prev.round() as i64),
                    (*latest_total as f64) / avg_prev
                ),
            }));
        }
    }
    anomalies
}

fn empty_calendar_day(date: String) -> CalendarEventDay {
    CalendarEventDay {
        date,
        income: 0.0,
        expense: 0.0,
        transfer_in: 0.0,
        transfer_out: 0.0,
        net: 0.0,
        count: 0,
        bills: Vec::new(),
    }
}

fn build_calendar_recurring_projections(
    recurring_rules: &[RecurringRuleInput],
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Vec<CalendarRecurringProjection> {
    let mut projections = Vec::new();
    for rule in recurring_rules {
        let Some(mut current) = parse_bill_date_prefix(&rule.next_date) else {
            continue;
        };
        let interval = recurring_interval_days(&rule.frequency);
        for _ in 0..50 {
            if current > end_date {
                break;
            }
            if current >= start_date {
                projections.push(CalendarRecurringProjection {
                    date: format_date(current),
                    projection_type: "recurring_projection".to_string(),
                    name: rule.name.clone(),
                    amount: cents_to_yuan(yuan_to_cents_lossy(&rule.amount_yuan)),
                    bill_type: rule.bill_type.clone(),
                    frequency: rule.frequency.clone(),
                    recurring_id: rule.id,
                });
            }
            current += Duration::days(interval);
        }
    }
    projections
}

fn recurring_interval_days(frequency: &str) -> i64 {
    match frequency {
        "weekly" => 7,
        "biweekly" => 14,
        "monthly" => 30,
        "bimonthly" => 60,
        "quarterly" => 90,
        "semiannual" => 180,
        "annual" => 365,
        _ => 30,
    }
}

fn builtin_cny_based_rates() -> Vec<(&'static str, f64)> {
    vec![
        ("USD", 0.139),
        ("EUR", 0.128),
        ("GBP", 0.110),
        ("JPY", 20.76),
        ("HKD", 1.087),
        ("KRW", 183.33),
        ("AUD", 0.211),
        ("CAD", 0.189),
        ("SGD", 0.186),
        ("TWD", 4.35),
        ("MYR", 0.646),
        ("THB", 4.86),
        ("VND", 3425.0),
        ("CHF", 0.123),
        ("NZD", 0.231),
    ]
}

fn format_python_round_rate(value: f64) -> String {
    let rounded = (value * 1_000_000.0).round() / 1_000_000.0;
    let mut text = format!("{rounded:.6}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    if !text.contains('.') {
        text.push_str(".0");
    }
    text
}

fn chinese_currency_name_map() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("人民币", "CNY"),
        ("美元", "USD"),
        ("欧元", "EUR"),
        ("英镑", "GBP"),
        ("日元", "JPY"),
        ("港币", "HKD"),
        ("韩元", "KRW"),
        ("韩国元", "KRW"),
        ("澳大利亚元", "AUD"),
        ("澳币", "AUD"),
        ("加拿大元", "CAD"),
        ("加拿大币", "CAD"),
        ("新加坡元", "SGD"),
        ("新加坡币", "SGD"),
        ("新台币", "TWD"),
        ("林吉特", "MYR"),
        ("马来币", "MYR"),
        ("泰国铢", "THB"),
        ("泰币", "THB"),
        ("越南盾", "VND"),
        ("瑞士法郎", "CHF"),
        ("新西兰元", "NZD"),
        ("纽元", "NZD"),
    ])
}

fn value_is_non_empty_array(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
}

fn value_is_non_empty_object(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_object)
        .is_some_and(|items| !items.is_empty())
}
