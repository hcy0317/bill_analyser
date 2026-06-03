// 中文导读：当前导入解析映射 helper，只服务 /api/bills/import/v2 当前路径。
// 维护重点：只保留 column-mapped 文本导入和当前 DTO 转换，不恢复历史读取/迁移入口。

#[tracing::instrument(level = "debug", skip_all)]
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
    let has_amount = headers
        .iter()
        .any(|header| header.contains("金额") || header == "amount" || header.contains("source_amount"));
    has_date && has_amount
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
    for row in rows.iter().skip(usize::from(has_header)) {
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

fn user_id_i64_value(user_id: UserId) -> Result<i64, ImportV2RouteResponse> {
    i64::try_from(user_id.get())
        .map_err(|_| import_v2_error_response(400, "user id exceeds PostgreSQL BIGINT range"))
}
