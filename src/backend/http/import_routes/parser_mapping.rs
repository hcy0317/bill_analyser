// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

fn resolve_import_file_parser_id(requested_parser: &str, filename: &str, content: &str) -> String {
    let requested_parser = requested_parser.trim().to_ascii_lowercase();
    if !requested_parser.is_empty() && requested_parser != "auto" {
        return requested_parser;
    }
    let probe = format!("{} {}", filename.to_ascii_lowercase(), content);
    for (keyword, parser_id) in [
        ("微信", "wechat"),
        ("wechat", "wechat"),
        ("支付宝", "alipay"),
        ("alipay", "alipay"),
        ("农业银行", "abc"),
        ("abc", "abc"),
        ("工商银行", "icbc"),
        ("icbc", "icbc"),
        ("建设银行", "ccb"),
        ("ccb", "ccb"),
        ("招商银行", "cmb"),
        ("cmb", "cmb"),
        ("民生银行", "cmbc"),
        ("cmbc", "cmbc"),
    ] {
        if probe.contains(keyword) {
            return parser_id.to_string();
        }
    }
    "rust-import".to_string()
}

fn parse_standard_bills_from_csv_text(
    text: &str,
    parser_id: &str,
    require_known_headers: bool,
) -> ParsedCsvBills {
    let Some((start_line, delimiter)) = detect_csv_table_start(text, require_known_headers) else {
        return ParsedCsvBills::default();
    };
    let csv_text = text.lines().skip(start_line).collect::<Vec<_>>().join("\n");
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .delimiter(delimiter as u8)
        .from_reader(csv_text.as_bytes());
    let headers = match reader.headers() {
        Ok(headers) => headers.clone(),
        Err(_) => return ParsedCsvBills::default(),
    };
    if require_known_headers && !looks_like_import_headers(headers.iter()) {
        return ParsedCsvBills::default();
    }
    let mut raw_bills = Vec::new();
    for record in reader.records().filter_map(Result::ok) {
        let mut raw_bill = RawBill::default();
        for (index, header) in headers.iter().enumerate() {
            let Some(value) = record.get(index) else {
                continue;
            };
            apply_csv_header_to_raw_bill(&mut raw_bill, header, value);
        }
        if !raw_bill.date.trim().is_empty()
            || !raw_bill.trade_time.trim().is_empty()
            || !raw_bill.amount.trim().is_empty()
        {
            raw_bills.push(raw_bill);
        }
    }
    ParsedCsvBills {
        bills: post_process_raw_bills(parser_id, &raw_bills),
    }
}

fn detect_csv_table_start(text: &str, require_known_headers: bool) -> Option<(usize, char)> {
    for (index, line) in text.lines().enumerate() {
        let Some(delimiter) = detect_delimiter_for_line(line, None) else {
            continue;
        };
        let headers = split_preview_line(line, delimiter);
        if require_known_headers {
            if looks_like_import_headers(headers.iter().map(String::as_str)) {
                return Some((index, delimiter));
            }
        } else if headers.len() > 1 {
            return Some((index, delimiter));
        }
    }
    None
}

fn looks_like_import_headers<'a>(headers: impl Iterator<Item = &'a str>) -> bool {
    let headers = headers.map(normalized_header_key).collect::<Vec<_>>();
    let has_date = headers.iter().any(|header| {
        header.contains("交易时间")
            || header.contains("交易日期")
            || header.contains("记账日期")
            || header == "date"
            || header == "time"
            || header.contains("trade_time")
    });
    let has_amount = headers.iter().any(|header| {
        header.contains("金额") || header == "amount" || header.contains("source_amount")
    });
    has_date && has_amount
}

fn apply_csv_header_to_raw_bill(raw_bill: &mut RawBill, header: &str, value: &str) {
    let header = normalized_header_key(header);
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    if header.contains("交易时间")
        || header.contains("交易日期")
        || header.contains("记账日期")
        || header == "date"
        || header == "time"
        || header.contains("trade_time")
    {
        raw_bill.trade_time = value.to_string();
    } else if header.contains("收支类型")
        || header.contains("收/支")
        || header.contains("交易类型")
        || header == "type"
        || header.contains("trade_type")
        || header.contains("transaction_type")
    {
        raw_bill.transaction_type = value.to_string();
    } else if header.contains("金额") || header == "amount" || header.contains("source_amount") {
        raw_bill.amount = value.to_string();
    } else if header.contains("商品") || header.contains("商品说明") || header == "goods" {
        raw_bill.goods = value.to_string();
    } else if header.contains("备注") || header.contains("说明") || header == "remark" {
        raw_bill.remark = value.to_string();
    } else if header.contains("摘要") || header == "summary" || header == "abstract" {
        raw_bill.summary = value.to_string();
    } else if header.contains("交易对方") || header.contains("对方") || header == "counterparty"
    {
        raw_bill.counterparty = value.to_string();
    } else if header.contains("商户") || header == "merchant" {
        raw_bill.merchant = value.to_string();
    } else if header.contains("店铺") || header == "shop" {
        raw_bill.shop = value.to_string();
    } else if header.contains("支付方式")
        || header.contains("收/付款方式")
        || header == "payment_method"
        || header == "account"
    {
        raw_bill.payment_method = value.to_string();
    } else if header.contains("交易分类") || header == "category" {
        raw_bill.original_category = value.to_string();
    } else if header.contains("交易单号") || header == "transaction_id" || header == "order_id"
    {
        raw_bill.transaction_id = value.to_string();
    } else if header.contains("商家订单号") || header == "merchant_id" {
        raw_bill.merchant_id = value.to_string();
    } else if header.contains("状态") || header == "status" {
        raw_bill.status = value.to_string();
    } else if raw_bill.description.is_empty() {
        raw_bill.description = value.to_string();
    }
}

fn normalized_header_key(header: &str) -> String {
    header
        .trim_matches(|ch: char| ch == '\u{feff}' || ch == '"' || ch == '\'')
        .trim()
        .replace([' ', '\t', '\r', '\n'], "")
        .to_ascii_lowercase()
}

fn standard_bills_from_temp_path_payload(
    object: &Map<String, Value>,
    temp_path: &str,
    user_id: UserId,
    session_id: &str,
) -> Result<Vec<StandardBill>, ImportV2RouteResponse> {
    let path = validate_import_temp_path(temp_path, user_id, Some(session_id))?;
    let text = read_import_temp_text(&path)?;
    standard_bills_from_column_mapped_text(object, &text)
}

fn standard_bills_from_column_mapped_text(
    object: &Map<String, Value>,
    text: &str,
) -> Result<Vec<StandardBill>, ImportV2RouteResponse> {
    let delimiter = first_text_from_object(object, &["delimiter"]);
    let mut rows = csv_rows_from_text(text, delimiter.as_deref())?;
    let column_mapping = column_mapping_from_payload(object)?;
    if !column_mapping.contains_key(&1) || !column_mapping.contains_key(&8) {
        return Err(import_v2_error_response(
            400,
            "Column mapping requires transaction time and amount columns",
        ));
    }
    let type_mapping = transaction_type_mapping_from_payload(object);
    let has_header =
        bool_field_from_object(object, &["has_header_line", "hasHeaderLine"]).unwrap_or(false);
    if has_header {
        if let Some(header_index) = rows
            .iter()
            .position(|row| looks_like_import_headers(row.iter().map(String::as_str)))
        {
            rows = rows.split_off(header_index);
        }
    }
    let mut bills = Vec::new();
    for row in rows.iter().skip(has_header as usize) {
        let Some((raw_bill, main_category, sub_category)) =
            raw_bill_from_column_mapped_row(row, &column_mapping, &type_mapping)
        else {
            continue;
        };
        let mut parsed = post_process_raw_bills("generic", &[raw_bill]);
        for bill in &mut parsed {
            bill.main_category = main_category.clone();
            bill.sub_category = sub_category.clone();
        }
        bills.extend(parsed);
    }
    Ok(bills)
}

fn column_mapping_from_payload(
    object: &Map<String, Value>,
) -> Result<HashMap<i64, usize>, ImportV2RouteResponse> {
    let Some(mapping) = first_value(
        object,
        &["column_mapping", "columnMapping", "dataColumnMapping"],
    ) else {
        return Err(import_v2_error_response(400, "Missing column_mapping"));
    };
    let mapping_value = value_or_json_object(mapping)?;
    let mut result = HashMap::new();
    for (key, value) in &mapping_value {
        let Some(column_type) = key.parse::<i64>().ok().filter(|value| *value > 0) else {
            continue;
        };
        let Some(column_index) = value_to_i64(value)
            .filter(|value| *value >= 0)
            .and_then(|value| usize::try_from(value).ok())
        else {
            continue;
        };
        result.insert(column_type, column_index);
    }
    if result.is_empty() {
        return Err(import_v2_error_response(400, "Invalid column_mapping"));
    }
    Ok(result)
}

fn transaction_type_mapping_from_payload(object: &Map<String, Value>) -> HashMap<String, String> {
    let Some(mapping) = first_value(
        object,
        &["transaction_type_mapping", "transactionTypeMapping"],
    ) else {
        return HashMap::new();
    };
    let Ok(mapping_value) = value_or_json_object(mapping) else {
        return HashMap::new();
    };
    mapping_value
        .iter()
        .filter_map(|(source, target)| {
            let mapped = value_to_i64(target).and_then(transaction_type_label_from_i64)?;
            Some((source.trim().to_string(), mapped.to_string()))
        })
        .collect()
}

fn value_or_json_object(value: &Value) -> Result<Map<String, Value>, ImportV2RouteResponse> {
    if let Some(object) = value.as_object() {
        return Ok(object.clone());
    }
    if let Some(text) = value.as_str() {
        let parsed = serde_json::from_str::<Value>(text)
            .map_err(|_| import_v2_error_response(400, "Invalid request"))?;
        if let Some(object) = parsed.as_object() {
            return Ok(object.clone());
        }
    }
    Err(import_v2_error_response(400, "Invalid request"))
}

fn raw_bill_from_column_mapped_row(
    row: &[String],
    column_mapping: &HashMap<i64, usize>,
    type_mapping: &HashMap<String, String>,
) -> Option<(RawBill, String, String)> {
    let mut raw_bill = RawBill {
        trade_time: mapped_column_text(row, column_mapping, 1),
        transaction_type: mapped_column_text(row, column_mapping, 3),
        amount: mapped_column_text(row, column_mapping, 8),
        payment_method: mapped_column_text(row, column_mapping, 6),
        description: mapped_column_text(row, column_mapping, 14),
        original_category: mapped_column_text(row, column_mapping, 4),
        ..RawBill::default()
    };
    if let Some(mapped_type) = type_mapping.get(raw_bill.transaction_type.trim()) {
        raw_bill.transaction_type = mapped_type.clone();
    }
    if raw_bill.description.trim().is_empty() {
        raw_bill.description = first_non_empty_text([
            mapped_column_text(row, column_mapping, 4),
            mapped_column_text(row, column_mapping, 5),
            raw_bill.payment_method.clone(),
        ]);
    }
    if raw_bill.trade_time.trim().is_empty() || raw_bill.amount.trim().is_empty() {
        return None;
    }
    let main_category = mapped_column_text(row, column_mapping, 4);
    let sub_category = mapped_column_text(row, column_mapping, 5);
    Some((raw_bill, main_category, sub_category))
}

fn mapped_column_text(
    row: &[String],
    column_mapping: &HashMap<i64, usize>,
    column_type: i64,
) -> String {
    column_mapping
        .get(&column_type)
        .and_then(|index| row.get(*index))
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

fn transaction_type_label_from_i64(value: i64) -> Option<&'static str> {
    match value {
        2 => Some("收入"),
        3 => Some("支出"),
        4 => Some("转账"),
        5 => Some("投资"),
        _ => None,
    }
}

fn first_non_empty_text(values: impl IntoIterator<Item = String>) -> String {
    values
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .unwrap_or_default()
}

fn now_text() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}

fn normalize_config_text(value: &str) -> String {
    value
        .trim()
        .replace([' ', '\t', '\r', '\n'], "")
        .to_ascii_lowercase()
}

fn config_text(value: &Value, keys: &[&str]) -> Option<String> {
    value
        .as_object()
        .and_then(|object| config_text_from_object(object, keys))
}

fn config_text_from_object(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    first_value(object, keys)
        .and_then(value_to_text)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn config_id_value(config: &Value) -> Option<i64> {
    config.as_object().and_then(|object| {
        first_value(object, &["id", "configId", "config_id"])
            .and_then(value_to_i64)
            .filter(|value| *value > 0)
    })
}

fn bool_value(config: &Value, key: &str) -> Option<bool> {
    config
        .as_object()
        .and_then(|object| object.get(key))
        .and_then(value_to_bool)
}

fn value_to_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(value) => Some(*value),
        Value::Number(number) => number.as_i64().map(|value| value != 0),
        Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "y" => Some(true),
            "false" | "0" | "no" | "n" => Some(false),
            _ => None,
        },
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn string_array_field_from_object(
    object: &Map<String, Value>,
    keys: &[&str],
) -> Option<Vec<String>> {
    first_value(object, keys).and_then(|value| match value {
        Value::Array(values) => Some(
            values
                .iter()
                .filter_map(value_to_text)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>(),
        ),
        Value::String(text) => Some(
            text.split([',', '|'])
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>(),
        ),
        _ => None,
    })
}

fn config_string_array(config: &Value, keys: &[&str]) -> Option<Vec<String>> {
    config
        .as_object()
        .and_then(|object| string_array_field_from_object(object, keys))
}

fn header_signature_from_headers(headers: &[String]) -> String {
    headers
        .iter()
        .map(|header| header.trim())
        .filter(|header| !header.is_empty())
        .collect::<Vec<_>>()
        .join("|")
}

fn normalize_learning_match_value(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_ascii_lowercase()
}

fn legacy_bill_type(object: &Map<String, Value>) -> String {
    first_value(object, &["type", "transaction_type", "transactionType"])
        .and_then(value_to_preview_type_text)
        .unwrap_or_else(|| "支出".to_string())
}

fn legacy_bill_amount(object: &Map<String, Value>) -> Option<f64> {
    first_value(
        object,
        &[
            "sourceAmount",
            "source_amount",
            "destinationAmount",
            "destination_amount",
        ],
    )
    .and_then(value_to_f64)
    .map(|value| value / 100.0)
    .or_else(|| first_value(object, &["amount"]).and_then(value_to_f64))
}

fn legacy_confirm_amount_for_type(bill_type: &str, amount: f64) -> f64 {
    let amount = amount.abs();
    if matches!(bill_type.trim(), "支出" | "expense") {
        -amount
    } else {
        amount
    }
}

fn legacy_bill_date(object: &Map<String, Value>) -> String {
    if let Some(date) = first_text_from_object(
        object,
        &["timeText", "date", "transaction_time", "tradeTime"],
    ) {
        return date.split_whitespace().next().unwrap_or(&date).to_string();
    }
    if let Some(timestamp) = first_value(object, &["time"]).and_then(value_to_i64) {
        if let Some(datetime) = chrono::DateTime::from_timestamp(timestamp, 0) {
            return datetime.date_naive().format("%Y-%m-%d").to_string();
        }
    }
    Utc::now().date_naive().format("%Y-%m-%d").to_string()
}

fn user_id_i64_value(user_id: UserId) -> Result<i64, ImportV2RouteResponse> {
    i64::try_from(user_id.get())
        .map_err(|_| import_v2_error_response(400, "user id exceeds sqlite integer range"))
}

