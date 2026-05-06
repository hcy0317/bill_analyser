use crate::{
    error::{ErrorCode, RuntimeError},
    primitives::{parse_bill_datetime, Money, TransactionType, UtcOffsetMinutes},
};
use serde::Serialize;
use serde_json::Value;

const DEFAULT_UTC_OFFSET_MINUTES: i32 = 480;
const FORMULA_PREFIXES: &[char] = &['=', '+', '-', '@'];
pub const EXPORT_COLUMNS: &[(&str, &str)] = &[
    ("date", "date"),
    ("type", "type"),
    ("amount", "amount"),
    ("counterparty", "counterparty"),
    ("description", "description"),
    ("payment_method", "payment_method"),
    ("main_category", "main_category"),
    ("sub_category", "sub_category"),
    ("source_account_id", "source_account_id"),
    ("destination_account_id", "destination_account_id"),
    ("destination_amount", "destination_amount"),
];
pub const EXPORT_TEXT_KEYS: &[&str] = &[
    "date",
    "type",
    "counterparty",
    "description",
    "payment_method",
    "main_category",
    "sub_category",
];

#[derive(Debug, Clone, PartialEq)]
pub struct BackendTransactionView {
    pub id: String,
    pub time_sequence_id: Option<String>,
    pub transaction_type: TransactionType,
    pub category_id: Option<String>,
    pub main_category: String,
    pub sub_category: String,
    pub date: String,
    pub amount: Money,
    pub destination_amount: Option<Money>,
    pub source_account_id: Option<i64>,
    pub destination_account_id: Option<i64>,
    pub utc_offset: UtcOffsetMinutes,
    pub hide_amount: bool,
    pub tag_ids: Vec<String>,
    pub tags: Vec<FrontendTransactionTag>,
    pub category: Option<Value>,
    pub source_account: Option<Value>,
    pub destination_account: Option<Value>,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendTransactionTag {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendTransactionView {
    pub id: String,
    pub time_sequence_id: String,
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    pub category_id: String,
    pub category_name: String,
    pub sub_category_name: String,
    pub time: i64,
    pub utc_offset: i32,
    pub source_account_id: String,
    pub destination_account_id: String,
    pub amount: i64,
    pub source_amount: i64,
    pub destination_amount: i64,
    pub hide_amount: bool,
    pub tag_ids: Vec<String>,
    pub tags: Vec<FrontendTransactionTag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_account: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_account: Option<Value>,
    pub comment: String,
    pub editable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gregorian_calendar_year_dash_month_dash_day: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gregorian_calendar_day_of_month: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_day_of_week: Option<u32>,
}

pub fn backend_transaction_type_name(transaction_type: TransactionType) -> &'static str {
    transaction_type.backend_name()
}

pub fn frontend_transaction_type_from_backend(
    raw_value: &str,
) -> Result<TransactionType, RuntimeError> {
    match raw_value {
        "收入" => Ok(TransactionType::Income),
        "支出" => Ok(TransactionType::Expense),
        "转账" => Ok(TransactionType::Transfer),
        "投资" => Ok(TransactionType::Investment),
        _ => Err(RuntimeError::new(
            ErrorCode::InvalidInput,
            "invalid backend transaction type",
        )),
    }
}

pub fn signed_backend_amount(transaction_type: TransactionType, source_amount: Money) -> Money {
    if matches!(
        transaction_type,
        TransactionType::Expense | TransactionType::Transfer | TransactionType::Investment
    ) && source_amount.is_positive()
    {
        source_amount
            .checked_negated()
            .expect("positive money values can always be negated")
    } else {
        source_amount
    }
}

pub fn frontend_transaction_from_backend(bill: &BackendTransactionView) -> FrontendTransactionView {
    let parsed_date = parse_bill_datetime(&bill.date);
    let time = parsed_date
        .map(|date| unix_seconds_from_local_bill_datetime(date.inner(), bill.utc_offset.as_i32()))
        .unwrap_or(0);
    let date_fields = parsed_date.map(|date| {
        (
            date.inner().format("%Y-%m-%d").to_string(),
            date.day_of_month(),
            date.display_day_of_week(),
        )
    });
    let amount = cents_abs_i64(bill.amount);
    let destination_amount = match bill.destination_amount {
        Some(value) if value != Money::ZERO => value,
        _ => bill.amount,
    };
    let tag_ids = if bill.tags.is_empty() {
        bill.tag_ids.clone()
    } else {
        bill.tags.iter().map(|tag| tag.id.clone()).collect()
    };

    FrontendTransactionView {
        id: bill.id.clone(),
        time_sequence_id: bill
            .time_sequence_id
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| bill.id.clone()),
        transaction_type: bill.transaction_type,
        category_id: bill.category_id.clone().unwrap_or_else(|| "0".to_string()),
        category_name: bill.main_category.clone(),
        sub_category_name: bill.sub_category.clone(),
        time,
        utc_offset: bill.utc_offset.as_i32(),
        source_account_id: account_id_string(bill.source_account_id),
        destination_account_id: account_id_string(bill.destination_account_id),
        amount,
        source_amount: amount,
        destination_amount: cents_abs_i64(destination_amount),
        hide_amount: bill.hide_amount,
        tag_ids,
        tags: bill.tags.clone(),
        category: bill.category.clone(),
        source_account: bill.source_account.clone(),
        destination_account: bill.destination_account.clone(),
        comment: bill.description.clone(),
        editable: true,
        gregorian_calendar_year_dash_month_dash_day: date_fields
            .as_ref()
            .map(|fields| fields.0.clone()),
        gregorian_calendar_day_of_month: date_fields.as_ref().map(|fields| fields.1),
        display_day_of_week: date_fields.map(|fields| fields.2),
    }
}

pub fn transaction_list_type_filter(raw_value: Option<&str>) -> Option<String> {
    let value = raw_value?.trim();
    if value.is_empty() {
        return None;
    }
    if let Ok(code) = value.parse::<i32>() {
        return match code {
            0 => None,
            2 => Some("收入".to_string()),
            3 => Some("支出".to_string()),
            4 => Some("转账".to_string()),
            5 => Some("投资".to_string()),
            _ => None,
        };
    }
    Some(value.to_string())
}

pub fn month_date_range(year: i32, month: u32) -> Result<(String, String), RuntimeError> {
    if !(1..=12).contains(&month) {
        return Err(RuntimeError::new(ErrorCode::InvalidInput, "invalid month"));
    }
    let start = format!("{year:04}-{month:02}-01");
    let (end_year, end_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let end = format!("{end_year:04}-{end_month:02}-01");
    Ok((start, end))
}

pub fn serialize_export_cell(key: &str, value: impl ToString) -> String {
    serialize_optional_export_cell(key, Some(value))
}

pub fn serialize_optional_export_cell<T: ToString>(key: &str, value: Option<T>) -> String {
    let serialized = value.map(|value| value.to_string()).unwrap_or_default();
    if EXPORT_TEXT_KEYS.contains(&key) && is_formula_like_export_cell(&serialized) {
        return format!("'{serialized}");
    }
    serialized
}

pub fn is_formula_like_export_cell(value: &str) -> bool {
    value
        .trim_start()
        .chars()
        .next()
        .is_some_and(|first| FORMULA_PREFIXES.contains(&first))
}

fn unix_seconds_from_local_bill_datetime(
    date: chrono::NaiveDateTime,
    utc_offset_minutes: i32,
) -> i64 {
    date.and_utc().timestamp() - i64::from(utc_offset_minutes) * 60
}

fn cents_abs_i64(amount: Money) -> i64 {
    let cents = i128::from(amount.to_cents());
    if cents < 0 {
        (-cents) as i64
    } else {
        cents as i64
    }
}

fn account_id_string(value: Option<i64>) -> String {
    match value {
        Some(value) if value > 0 => value.to_string(),
        _ => "0".to_string(),
    }
}

impl Default for BackendTransactionView {
    fn default() -> Self {
        Self {
            id: String::new(),
            time_sequence_id: None,
            transaction_type: TransactionType::Expense,
            category_id: None,
            main_category: String::new(),
            sub_category: String::new(),
            date: String::new(),
            amount: Money::ZERO,
            destination_amount: None,
            source_account_id: None,
            destination_account_id: None,
            utc_offset: UtcOffsetMinutes::new(DEFAULT_UTC_OFFSET_MINUTES),
            hide_amount: false,
            tag_ids: Vec::new(),
            tags: Vec::new(),
            category: None,
            source_account: None,
            destination_account: None,
            description: String::new(),
        }
    }
}
