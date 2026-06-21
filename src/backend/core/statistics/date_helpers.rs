fn first_day(year: i32, month: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, 1).expect("valid first day")
}

fn add_months(date: NaiveDate, months: u32) -> NaiveDate {
    let zero_based = date.month0() + months;
    let year = date.year() + (zero_based / 12) as i32;
    let month = (zero_based % 12) + 1;
    first_day(year, month)
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_i64_text(text: &str, error: &str) -> Result<i64, StatisticsContractError> {
    text.parse::<i64>()
        .map_err(|exc| StatisticsContractError::new(error, format!("{error}: {exc}")))
}

fn clean_year_month_text(text: &str) -> String {
    text.trim().replace('-', "")
}

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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
    let mut amount = bill.amount_cents;
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

#[tracing::instrument(level = "debug", skip_all)]
fn apply_asset_trend_bill(current_balances: &mut BTreeMap<i64, i64>, bill: &StatisticsBillInput) {
    let amount = bill.amount_cents.abs();
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
                    .destination_amount_cents
                    .filter(|amount| *amount != 0)
                    .unwrap_or(amount)
                    .abs();
                *balance += destination_amount;
            }
        }
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_bill_date_prefix(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.get(0..10)?, "%Y-%m-%d").ok()
}

#[tracing::instrument(level = "debug", skip_all)]
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

fn display_amount_from_cents(cents: i64) -> f64 {
    round_money((cents as f64) / 100.0)
}

fn round_money(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn average_total_cents(total_cents: i64, count: usize) -> i64 {
    if count == 0 {
        0
    } else {
        (total_cents as f64 / count as f64).round() as i64
    }
}

fn value_as_i64(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok())),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        Value::Bool(_) | Value::Array(_) | Value::Object(_) | Value::Null => None,
    }
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
