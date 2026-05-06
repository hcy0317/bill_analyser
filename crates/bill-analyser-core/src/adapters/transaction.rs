use crate::{
    error::{ErrorCode, RuntimeError},
    primitives::{parse_bill_datetime, Money, TransactionType, UtcOffsetMinutes},
};
use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::{Map, Number, Value};

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
pub const BILL_CREATE_COLUMNS: &[&str] = &[
    "user_id",
    "date",
    "type",
    "amount",
    "counterparty",
    "description",
    "payment_method",
    "main_category",
    "sub_category",
    "batch_id",
    "hash",
    "created_at",
    "updated_at",
    "source_account_id",
    "destination_account_id",
    "destination_amount",
    "created_from_template",
    "created_from_recurring",
    "import_history_id",
];
pub const BILL_UPDATE_COLUMNS: &[&str] = &[
    "date",
    "type",
    "amount",
    "counterparty",
    "description",
    "payment_method",
    "main_category",
    "sub_category",
    "batch_id",
    "hash",
    "source_account_id",
    "destination_account_id",
    "destination_amount",
    "created_from_template",
    "created_from_recurring",
    "import_history_id",
];
pub const ROUTE_ALLOWED_BATCH_UPDATE_FIELDS: &[&str] = &[
    "date",
    "type",
    "amount",
    "counterparty",
    "description",
    "payment_method",
    "main_category",
    "sub_category",
    "source_account_id",
    "destination_account_id",
    "destination_amount",
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillMutationMetadata {
    pub category_id: String,
    pub source_account_id: i64,
    pub destination_account_id: i64,
    pub tag_ids: Vec<i64>,
    pub auto_invest_account: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BillAccountSyncSnapshot {
    pub source_account_id: Option<i64>,
    pub destination_account_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountBalanceBill {
    pub transaction_type: TransactionType,
    pub amount: Money,
    pub destination_amount: Money,
    pub source_account_id: Option<i64>,
    pub destination_account_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BatchUpdateResponse {
    pub updated_count: usize,
    pub failed_count: usize,
    pub failed_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendBillUpdateSnapshot {
    pub transaction_type: String,
    pub source_account_id: Option<i64>,
    pub destination_account_id: Option<i64>,
    pub destination_amount: Option<Money>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreateResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_index: Option<usize>,
    pub created_count: usize,
    pub items: Vec<Value>,
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RouteResponseContract {
    pub status_code: u16,
    pub body: Value,
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

pub fn frontend_transaction_mutation_to_backend(
    frontend_data: &Value,
    _utc_offset: UtcOffsetMinutes,
) -> Result<(Map<String, Value>, BillMutationMetadata), RuntimeError> {
    let transaction_type = frontend_transaction_type_from_value(frontend_data.get("type"))?;
    let source_amount = Money::from_cents(frontend_amount_cents(frontend_data.get("sourceAmount")));
    let destination_amount = Money::from_cents(frontend_amount_cents(
        frontend_data.get("destinationAmount"),
    ));
    let amount = match transaction_type {
        Some(
            TransactionType::Expense | TransactionType::Transfer | TransactionType::Investment,
        ) if source_amount.is_positive() => source_amount.checked_negated()?,
        _ => source_amount,
    };
    let source_account_id = value_to_i64(frontend_data.get("sourceAccountId"), 0)?;
    let destination_account_id = value_to_i64(frontend_data.get("destinationAccountId"), 0)?;
    let unix_time = normalize_frontend_unix_time(frontend_data.get("time"));
    let date = if unix_time == 0 {
        String::new()
    } else {
        frontend_unix_time_to_backend_date(
            unix_time,
            UtcOffsetMinutes::new(DEFAULT_UTC_OFFSET_MINUTES),
        )?
    };
    let description = value_string(
        frontend_data
            .get("comment")
            .or_else(|| frontend_data.get("remark")),
    )
    .unwrap_or_default();

    let mut backend_data = Map::new();
    backend_data.insert(
        "type".to_string(),
        Value::String(
            transaction_type
                .map(TransactionType::backend_name)
                .unwrap_or_default()
                .to_string(),
        ),
    );
    backend_data.insert("date".to_string(), Value::String(date));
    backend_data.insert("amount".to_string(), money_to_yuan_json(amount));
    backend_data.insert(
        "destination_amount".to_string(),
        money_to_yuan_json(destination_amount),
    );
    backend_data.insert(
        "source_account_id".to_string(),
        Value::Number(Number::from(source_account_id)),
    );
    backend_data.insert(
        "destination_account_id".to_string(),
        Value::Number(Number::from(destination_account_id)),
    );
    backend_data.insert("description".to_string(), Value::String(description));

    let metadata = BillMutationMetadata {
        category_id: metadata_string(frontend_data.get("categoryId")),
        source_account_id,
        destination_account_id,
        tag_ids: tag_ids_from_value(frontend_data.get("tagIds"))?,
        auto_invest_account: false,
    };
    Ok((backend_data, metadata))
}

pub fn apply_manual_create_defaults(
    backend_data: &mut Map<String, Value>,
    frontend_data: &Value,
    fallback_source_account_id: Option<i64>,
) -> Result<(), RuntimeError> {
    let description_fallback =
        first_non_empty_string(frontend_data, &["comment", "remark", "description"])
            .unwrap_or_default();
    let counterparty_fallback = first_non_empty_string(
        frontend_data,
        &[
            "counterparty",
            "payee",
            "merchant",
            "merchantName",
            "shopName",
            "targetAccountName",
        ],
    )
    .or_else(|| (!description_fallback.is_empty()).then_some(description_fallback.clone()))
    .or_else(|| non_empty_map_string(backend_data, "payment_method"))
    .unwrap_or_else(|| "手工录入".to_string());

    if non_empty_map_string(backend_data, "description").is_none() {
        backend_data.insert(
            "description".to_string(),
            Value::String(counterparty_fallback.clone()),
        );
    }
    if non_empty_map_string(backend_data, "counterparty").is_none() {
        backend_data.insert(
            "counterparty".to_string(),
            Value::String(counterparty_fallback),
        );
    }

    let source_account_id = map_i64(backend_data, "source_account_id")?;
    if source_account_id <= 0 {
        let fallback = fallback_source_account_id
            .filter(|value| *value > 0)
            .ok_or_else(|| RuntimeError::new(ErrorCode::InvalidInput, "No account available"))?;
        backend_data.insert(
            "source_account_id".to_string(),
            Value::Number(Number::from(fallback)),
        );
    }

    Ok(())
}

pub fn normalize_bill_create_aliases(backend_data: &mut Map<String, Value>) {
    if !backend_data.contains_key("payment_method") {
        if let Some(channel) = backend_data.remove("channel").filter(|value| {
            !value_string(Some(value))
                .unwrap_or_default()
                .trim()
                .is_empty()
        }) {
            backend_data.insert("payment_method".to_string(), channel);
        }
    }
    if !backend_data.contains_key("main_category") {
        if let Some(category) = backend_data.remove("category").filter(|value| {
            !value_string(Some(value))
                .unwrap_or_default()
                .trim()
                .is_empty()
        }) {
            backend_data.insert("main_category".to_string(), category);
        }
    }
}

pub fn apply_legacy_modify_preserved_fields(
    backend_data: &mut Map<String, Value>,
    frontend_data: &Value,
    old_bill: &BackendBillUpdateSnapshot,
) {
    if frontend_data.get("remark").is_some() && frontend_data.get("type").is_none() {
        let mut simple_update = Map::new();
        if let Some(remark) = value_string(frontend_data.get("remark")) {
            simple_update.insert("description".to_string(), Value::String(remark));
        }
        if let Some(comment) = value_string(frontend_data.get("comment")) {
            simple_update.insert("description".to_string(), Value::String(comment));
        }
        simple_update.insert(
            "type".to_string(),
            Value::String(old_bill.transaction_type.clone()),
        );
        *backend_data = simple_update;
    }

    if non_empty_map_string(backend_data, "type").is_none() {
        backend_data.insert(
            "type".to_string(),
            Value::String(old_bill.transaction_type.clone()),
        );
    }
    if !backend_data.contains_key("source_account_id") {
        backend_data.insert(
            "source_account_id".to_string(),
            Value::Number(Number::from(old_bill.source_account_id.unwrap_or(0))),
        );
    }
    if !backend_data.contains_key("destination_account_id") {
        backend_data.insert(
            "destination_account_id".to_string(),
            Value::Number(Number::from(old_bill.destination_account_id.unwrap_or(0))),
        );
    }
    if !backend_data.contains_key("destination_amount") {
        backend_data.insert(
            "destination_amount".to_string(),
            money_to_yuan_json(old_bill.destination_amount.unwrap_or(Money::ZERO)),
        );
    }
}

pub fn apply_create_category_contract(
    backend_data: &mut Map<String, Value>,
    resolved_category: Option<(&str, &str)>,
    rule_matched_category: Option<(&str, &str)>,
) {
    if let Some((main_category, sub_category)) = resolved_category.or(rule_matched_category) {
        backend_data.insert(
            "main_category".to_string(),
            Value::String(main_category.to_string()),
        );
        backend_data.insert(
            "sub_category".to_string(),
            Value::String(sub_category.to_string()),
        );
        return;
    }

    if non_empty_map_string(backend_data, "main_category").is_some() {
        return;
    }

    let bill_type =
        non_empty_map_string(backend_data, "type").unwrap_or_else(|| "支出".to_string());
    let (main_category, sub_category) = default_bill_category(&bill_type);
    backend_data.insert(
        "main_category".to_string(),
        Value::String(main_category.to_string()),
    );
    backend_data.insert(
        "sub_category".to_string(),
        Value::String(sub_category.to_string()),
    );
}

pub fn batch_create_transaction_items(
    payload: &Value,
) -> Result<Vec<&Map<String, Value>>, RuntimeError> {
    let transactions = match payload {
        Value::Object(object) => object
            .get("transactions")
            .filter(|value| python_json_truthy(value))
            .or_else(|| object.get("bills")),
        Value::Array(_) => Some(payload),
        _ => None,
    };
    let Some(Value::Array(items)) = transactions else {
        return Err(RuntimeError::new(
            ErrorCode::InvalidInput,
            "transactions is required",
        ));
    };
    if items.is_empty() {
        return Err(RuntimeError::new(
            ErrorCode::InvalidInput,
            "transactions is required",
        ));
    }

    let mut objects = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let Some(object) = item.as_object() else {
            return Err(RuntimeError::new(
                ErrorCode::InvalidInput,
                format!("transactions[{index}] must be an object"),
            ));
        };
        objects.push(object);
    }
    Ok(objects)
}

pub fn validate_bill_create_fields<'a>(
    fields: impl IntoIterator<Item = &'a str>,
) -> Result<(), RuntimeError> {
    validate_fields(
        fields,
        BILL_CREATE_COLUMNS,
        "unsupported bill create fields",
    )
}

pub fn validate_bill_update_fields<'a>(
    fields: impl IntoIterator<Item = &'a str>,
) -> Result<(), RuntimeError> {
    validate_fields(
        fields,
        BILL_UPDATE_COLUMNS,
        "unsupported bill update fields",
    )
}

pub fn validate_batch_route_update_fields<'a>(
    fields: impl IntoIterator<Item = &'a str>,
) -> Result<(), RuntimeError> {
    validate_fields(
        fields,
        ROUTE_ALLOWED_BATCH_UPDATE_FIELDS,
        "unsupported update fields",
    )
}

pub fn batch_update_response(
    success_count: usize,
    failed_count: usize,
    failed_ids: Vec<i64>,
) -> BatchUpdateResponse {
    BatchUpdateResponse {
        updated_count: success_count,
        failed_count,
        failed_ids,
    }
}

pub fn batch_create_success_response(items: Vec<Value>, ids: Vec<String>) -> BatchCreateResult {
    BatchCreateResult {
        failed_index: None,
        created_count: items.len(),
        items,
        ids,
    }
}

pub fn batch_create_success_route_response(
    items: Vec<Value>,
    ids: Vec<String>,
) -> RouteResponseContract {
    RouteResponseContract {
        status_code: 201,
        body: success_result_body(
            serde_json::to_value(batch_create_success_response(items, ids))
                .expect("batch create result should serialize"),
        ),
    }
}

pub fn batch_create_failure_response(
    failed_index: usize,
    created_items: Vec<Value>,
    created_ids: Vec<String>,
) -> BatchCreateResult {
    BatchCreateResult {
        failed_index: Some(failed_index),
        created_count: created_items.len(),
        items: created_items,
        ids: created_ids,
    }
}

pub fn batch_create_prepare_error_route_response(
    error: impl Into<String>,
    failed_index: usize,
) -> RouteResponseContract {
    RouteResponseContract {
        status_code: 400,
        body: error_result_body(error, batch_create_prepare_error_result(failed_index)),
    }
}

pub fn batch_create_persist_error_route_response(
    error: impl Into<String>,
    failed_index: usize,
    created_items: Vec<Value>,
    created_ids: Vec<String>,
) -> RouteResponseContract {
    RouteResponseContract {
        status_code: 500,
        body: error_result_body(
            error,
            serde_json::to_value(batch_create_failure_response(
                failed_index,
                created_items,
                created_ids,
            ))
            .expect("batch create failure result should serialize"),
        ),
    }
}

pub fn delete_bill_success_payload() -> Value {
    let mut payload = Map::new();
    payload.insert("success".to_string(), Value::Bool(true));
    payload.insert("result".to_string(), Value::Bool(true));
    payload.insert(
        "message".to_string(),
        Value::String("Bill deleted successfully".to_string()),
    );
    Value::Object(payload)
}

pub fn legacy_delete_bill_success_payload() -> Value {
    let mut payload = Map::new();
    payload.insert("success".to_string(), Value::Bool(true));
    Value::Object(payload)
}

pub fn legacy_modify_bill_success_payload(bill_id: impl ToString) -> Value {
    let mut result = Map::new();
    result.insert("id".to_string(), Value::String(bill_id.to_string()));
    let mut payload = Map::new();
    payload.insert("success".to_string(), Value::Bool(true));
    payload.insert("result".to_string(), Value::Object(result));
    Value::Object(payload)
}

pub fn batch_delete_success_payload(deleted_count: usize) -> Value {
    let mut result = Map::new();
    result.insert(
        "deleted_count".to_string(),
        Value::Number(Number::from(deleted_count)),
    );
    let mut payload = Map::new();
    payload.insert("success".to_string(), Value::Bool(true));
    payload.insert("result".to_string(), Value::Object(result));
    Value::Object(payload)
}

pub fn batch_update_balance_sync_account_ids<'a>(
    _updated_fields: impl IntoIterator<Item = &'a str>,
) -> Vec<i64> {
    Vec::new()
}

pub fn sync_account_ids_for_bill(snapshot: BillAccountSyncSnapshot) -> Vec<i64> {
    collect_account_ids([snapshot])
}

pub fn sync_account_ids_for_update(
    old_bill: BillAccountSyncSnapshot,
    new_bill: BillAccountSyncSnapshot,
) -> Vec<i64> {
    collect_account_ids([old_bill, new_bill])
}

pub fn sync_account_ids_for_batch_delete(bills: &[BillAccountSyncSnapshot]) -> Vec<i64> {
    collect_account_ids(bills.iter().copied())
}

pub fn calculate_account_balance_from_bills(
    account_id: i64,
    initial_balance: Money,
    bills: &[AccountBalanceBill],
    same_account_investment_pnl_correction: Money,
) -> Result<Money, RuntimeError> {
    let mut income = 0_i128;
    let mut expense = 0_i128;
    let mut transfer_out = 0_i128;
    let mut transfer_in = 0_i128;
    let mut investment_out = 0_i128;
    let mut investment_in = 0_i128;

    for bill in bills {
        if bill.source_account_id == Some(account_id) {
            match bill.transaction_type {
                TransactionType::Income => income += i128::from(bill.amount.to_cents()),
                TransactionType::Expense => expense += i128::from(bill.amount.to_cents()),
                TransactionType::Transfer => transfer_out += i128::from(bill.amount.to_cents()),
                TransactionType::Investment => investment_out += i128::from(bill.amount.to_cents()),
            }
        }
        if bill.destination_account_id == Some(account_id) {
            match bill.transaction_type {
                TransactionType::Transfer => {
                    transfer_in += i128::from(bill.destination_amount.to_cents());
                }
                TransactionType::Investment => {
                    investment_in += i128::from(bill.destination_amount.to_cents());
                }
                TransactionType::Income | TransactionType::Expense => {}
            }
        }
    }

    let balance = i128::from(initial_balance.to_cents()) + income - expense - transfer_out
        + transfer_in
        - investment_out
        + investment_in
        + i128::from(same_account_investment_pnl_correction.to_cents());
    money_from_i128_cents(balance)
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

fn frontend_unix_time_to_backend_date(
    unix_time: i64,
    utc_offset: UtcOffsetMinutes,
) -> Result<String, RuntimeError> {
    let utc_datetime = chrono::DateTime::from_timestamp(unix_time, 0).ok_or_else(|| {
        RuntimeError::new(ErrorCode::InvalidInput, "invalid frontend transaction time")
    })?;
    let local_datetime =
        utc_datetime.naive_utc() + chrono::Duration::minutes(i64::from(utc_offset.as_i32()));
    Ok(local_datetime.format("%Y-%m-%d %H:%M:%S").to_string())
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

fn frontend_transaction_type_from_value(
    raw_value: Option<&Value>,
) -> Result<Option<TransactionType>, RuntimeError> {
    let Some(value) = raw_value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    if value.as_str().is_some_and(|text| text.trim().is_empty()) {
        return Ok(None);
    }
    let type_code = value_to_i64(Some(value), 0)?;
    Ok(Some(match type_code {
        2 => TransactionType::Income,
        3 => TransactionType::Expense,
        4 => TransactionType::Transfer,
        5 => TransactionType::Investment,
        _ => TransactionType::Expense,
    }))
}

fn normalize_frontend_unix_time(raw_value: Option<&Value>) -> i64 {
    let Ok(mut normalized) = value_to_i64(raw_value, 0) else {
        return 0;
    };
    if normalized.abs() >= 10_i64.pow(11) {
        normalized /= 1000;
    }
    normalized
}

fn value_to_i64(raw_value: Option<&Value>, default: i64) -> Result<i64, RuntimeError> {
    let Some(value) = raw_value else {
        return Ok(default);
    };
    match value {
        Value::Null => Ok(default),
        Value::Bool(value) => Ok(i64::from(*value)),
        Value::Number(number) => parse_json_integer_text(&number.to_string()),
        Value::String(text) if text.trim().is_empty() => Ok(default),
        Value::String(text) => text
            .trim()
            .parse::<i64>()
            .map_err(|_| RuntimeError::new(ErrorCode::InvalidInput, "invalid integer value")),
        Value::Array(_) | Value::Object(_) => Err(RuntimeError::new(
            ErrorCode::InvalidInput,
            "invalid integer value",
        )),
    }
}

fn frontend_amount_cents(raw_value: Option<&Value>) -> i64 {
    value_to_i64(raw_value, 0).unwrap_or(0)
}

fn python_json_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(number) => !json_number_text_is_zero(&number.to_string()),
        Value::String(text) => !text.is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
    }
}

fn tag_ids_from_value(raw_value: Option<&Value>) -> Result<Vec<i64>, RuntimeError> {
    let Some(Value::Array(values)) = raw_value else {
        return Ok(Vec::new());
    };
    values
        .iter()
        .filter(|value| {
            value_string(Some(value)).is_some_and(|text| !text.trim().is_empty() && text != "0")
        })
        .map(|value| value_to_i64(Some(value), 0))
        .collect()
}

fn metadata_string(raw_value: Option<&Value>) -> String {
    value_string(raw_value)
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_default()
}

fn value_string(raw_value: Option<&Value>) -> Option<String> {
    match raw_value? {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

fn first_non_empty_string(frontend_data: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value_string(frontend_data.get(*key))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn non_empty_map_string(map: &Map<String, Value>, key: &str) -> Option<String> {
    value_string(map.get(key))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn map_i64(map: &Map<String, Value>, key: &str) -> Result<i64, RuntimeError> {
    value_to_i64(map.get(key), 0)
}

fn money_to_yuan_json(amount: Money) -> Value {
    serde_json::from_str(&amount.to_yuan_string())
        .expect("money yuan string should serialize as JSON number")
}

fn money_from_i128_cents(cents: i128) -> Result<Money, RuntimeError> {
    let cents = i64::try_from(cents)
        .map_err(|_| RuntimeError::new(ErrorCode::InvalidInput, "money amount is too large"))?;
    Ok(Money::from_cents(cents))
}

fn validate_fields<'a>(
    fields: impl IntoIterator<Item = &'a str>,
    allowed_fields: &[&str],
    message: &str,
) -> Result<(), RuntimeError> {
    let invalid_fields: BTreeSet<&str> = fields
        .into_iter()
        .filter(|field| !allowed_fields.contains(field))
        .collect();
    if invalid_fields.is_empty() {
        Ok(())
    } else {
        let invalid_fields = invalid_fields.into_iter().collect::<Vec<_>>().join(", ");
        Err(RuntimeError::new(
            ErrorCode::InvalidInput,
            format!("{message}: {invalid_fields}"),
        ))
    }
}

fn collect_account_ids(snapshots: impl IntoIterator<Item = BillAccountSyncSnapshot>) -> Vec<i64> {
    let mut ids = BTreeSet::new();
    for snapshot in snapshots {
        if let Some(source_id) = snapshot.source_account_id.filter(|value| *value > 0) {
            ids.insert(source_id);
        }
        if let Some(destination_id) = snapshot.destination_account_id.filter(|value| *value > 0) {
            ids.insert(destination_id);
        }
    }
    ids.into_iter().collect()
}

fn success_result_body(result: Value) -> Value {
    let mut body = Map::new();
    body.insert("success".to_string(), Value::Bool(true));
    body.insert("result".to_string(), result);
    Value::Object(body)
}

fn error_result_body(error: impl Into<String>, result: Value) -> Value {
    let mut body = Map::new();
    body.insert("success".to_string(), Value::Bool(false));
    body.insert("error".to_string(), Value::String(error.into()));
    body.insert("result".to_string(), result);
    Value::Object(body)
}

fn batch_create_prepare_error_result(failed_index: usize) -> Value {
    let mut result = Map::new();
    result.insert(
        "failedIndex".to_string(),
        Value::Number(Number::from(failed_index)),
    );
    result.insert("createdCount".to_string(), Value::Number(Number::from(0)));
    result.insert("items".to_string(), Value::Array(Vec::new()));
    Value::Object(result)
}

fn parse_json_integer_text(text: &str) -> Result<i64, RuntimeError> {
    let normalized = if let Some((whole, fraction)) = text.split_once('.') {
        if fraction.bytes().all(|byte| byte == b'0') {
            whole
        } else {
            return Err(RuntimeError::new(
                ErrorCode::InvalidInput,
                "invalid integer value",
            ));
        }
    } else {
        text
    };
    normalized
        .parse::<i64>()
        .map_err(|_| RuntimeError::new(ErrorCode::InvalidInput, "invalid integer value"))
}

fn json_number_text_is_zero(text: &str) -> bool {
    let text = text.strip_prefix('-').unwrap_or(text);
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    whole.bytes().all(|byte| byte == b'0') && fraction.bytes().all(|byte| byte == b'0')
}

fn default_bill_category(transaction_type: &str) -> (&'static str, &'static str) {
    match transaction_type {
        "收入" => ("工资", ""),
        "支出" => ("其他", "日常支出"),
        "转账" => ("转账", ""),
        "投资" => ("投资理财", "证券投资"),
        _ => ("其他", ""),
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
