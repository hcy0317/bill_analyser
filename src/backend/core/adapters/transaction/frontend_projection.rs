/// 把后端正式账单视图投影为前端交易列表和编辑页共用的响应结构。
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
        amount_cents: amount,
        source_amount_cents: amount,
        destination_amount_cents: cents_abs_i64(destination_amount),
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

/// 将交易列表 type 查询参数归一为后端筛选可识别的交易类型名。
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

/// 根据前端 0-based 月份参数生成整月闭区间日期字符串。
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

/// 生成导出单元格的 key=value 文本，并对公式样式内容加引号防注入。
pub fn serialize_export_cell(key: &str, value: impl ToString) -> String {
    serialize_optional_export_cell(key, Some(value))
}

/// 仅在可选值存在时生成导出单元格，避免把空值导出成误导性文本。
pub fn serialize_optional_export_cell<T: ToString>(key: &str, value: Option<T>) -> String {
    let serialized = value.map(|value| value.to_string()).unwrap_or_default();
    if EXPORT_TEXT_KEYS.contains(&key) && is_formula_like_export_cell(&serialized) {
        return format!("'{serialized}");
    }
    serialized
}

/// 判断导出文本是否可能被电子表格当作公式执行。
pub fn is_formula_like_export_cell(value: &str) -> bool {
    value
        .trim_start()
        .chars()
        .next()
        .is_some_and(|first| FORMULA_PREFIXES.contains(&first))
}
