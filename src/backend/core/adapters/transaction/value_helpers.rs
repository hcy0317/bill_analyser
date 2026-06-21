fn unix_seconds_from_local_bill_datetime(
    date: chrono::NaiveDateTime,
    utc_offset_minutes: i32,
) -> i64 {
    date.and_utc().timestamp() - i64::from(utc_offset_minutes) * 60
}

fn reconciliation_date_from_timestamp(seconds: i64) -> Option<String> {
    let utc_datetime = chrono::DateTime::from_timestamp(seconds, 0)?;
    Some(
        utc_datetime
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d")
            .to_string(),
    )
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

fn abs_cents_i128(amount: Money) -> i128 {
    let cents = i128::from(amount.to_cents());
    if cents < 0 {
        -cents
    } else {
        cents
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
        Value::Bool(_) => Err(RuntimeError::new(
            ErrorCode::InvalidInput,
            "invalid integer value",
        )),
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

fn frontend_amount_cents(raw_value: Option<&Value>) -> Result<i64, RuntimeError> {
    value_to_i64(raw_value, 0)
}

fn json_value_is_truthy(value: &Value) -> bool {
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

fn simple_route_error_response(
    status_code: u16,
    error: impl Into<String>,
) -> RouteResponseContract {
    let mut body = Map::new();
    body.insert("success".to_string(), Value::Bool(false));
    body.insert("error".to_string(), Value::String(error.into()));
    RouteResponseContract {
        status_code,
        body: Value::Object(body),
    }
}

fn windows_device_file_name(filename: &str) -> bool {
    let stem = filename.split('.').next().unwrap_or_default();
    WINDOWS_DEVICE_FILES.contains(&stem.to_ascii_uppercase().as_str())
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

fn path_suffix_lower(filename: &str) -> Option<String> {
    let (stem, extension) = filename.rsplit_once('.')?;
    if stem.is_empty() || extension.is_empty() {
        return None;
    }
    Some(format!(".{}", extension.to_ascii_lowercase()))
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
