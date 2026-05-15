
impl UserDataExportType {
    const fn delimiter(self) -> u8 {
        match self {
            Self::Csv => b',',
            Self::Tsv => b'\t',
        }
    }

    const fn content_type(self) -> &'static str {
        match self {
            Self::Csv => "text/csv; charset=utf-8",
            Self::Tsv => "text/tab-separated-values; charset=utf-8",
        }
    }

    const fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Tsv => "tsv",
        }
    }
}

fn normalize_user_data_export_type(file_type: &str) -> Result<UserDataExportType, AuthRestError> {
    match file_type.trim().to_ascii_lowercase().as_str() {
        "csv" => Ok(UserDataExportType::Csv),
        "tsv" => Ok(UserDataExportType::Tsv),
        _ => Err(AuthRestError::new(
            400,
            "Invalid request",
            "Unsupported export file type",
        )),
    }
}

fn build_user_data_export_filters(
    query: &HashMap<String, String>,
    categories: &[UserDataExportCategory],
) -> BillFilters {
    BillFilters {
        date_from: query
            .get("min_time")
            .and_then(|value| parse_export_timestamp_millis(value)),
        date_to: query
            .get("max_time")
            .and_then(|value| parse_export_timestamp_millis(value)),
        transaction_type: query
            .get("type")
            .and_then(|value| user_data_export_transaction_type(value)),
        keyword: query
            .get("keyword")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        amount_filter: query
            .get("amount_filter")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        account_ids: query
            .get("account_ids")
            .map(|value| parse_comma_separated_ints(value))
            .unwrap_or_default(),
        tag_ids: query
            .get("tag_ids")
            .map(|value| parse_comma_separated_ints(value))
            .unwrap_or_default(),
        categories: user_data_export_category_filters(
            query
                .get("category_ids")
                .map(|value| parse_comma_separated_ints(value))
                .unwrap_or_default(),
            categories,
        ),
        ..Default::default()
    }
}

fn user_data_export_transaction_type(raw_value: &str) -> Option<String> {
    let value = raw_value.trim();
    if value.is_empty() || value == "0" {
        return None;
    }
    if let Ok(code) = value.parse::<i64>() {
        return match code {
            2 => Some("收入".to_string()),
            3 => Some("支出".to_string()),
            4 => Some("转账".to_string()),
            5 => Some("投资".to_string()),
            _ => None,
        };
    }
    Some(value.to_string())
}

fn user_data_export_category_filters(
    category_ids: Vec<i64>,
    categories: &[UserDataExportCategory],
) -> Vec<BillCategoryFilter> {
    if category_ids.is_empty() {
        return Vec::new();
    }
    let category_map = categories
        .iter()
        .map(|category| (category.id, category))
        .collect::<BTreeMap<_, _>>();
    category_ids
        .into_iter()
        .filter_map(|category_id| category_map.get(&category_id))
        .map(|category| BillCategoryFilter {
            main: category.main_category.clone(),
            sub: Some(category.sub_category.clone()).filter(|value| !value.trim().is_empty()),
        })
        .collect()
}

fn render_user_data_export(
    bundle: &UserDataExportBundle,
    delimiter: u8,
) -> Result<String, csv::Error> {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());
    writer.write_record([
        "id",
        "date",
        "type",
        "amount",
        "main_category",
        "sub_category",
        "source_account",
        "destination_account",
        "counterparty",
        "payment_method",
        "description",
        "tags",
        "comment",
        "created_at",
        "updated_at",
    ])?;
    for bill in &bundle.bills {
        let bill_id = value_i64(bill.get("id")).unwrap_or(-1);
        writer.write_record([
            export_value(bill.get("id"), "id"),
            export_value(bill.get("date"), "date"),
            export_value(bill.get("type"), "type"),
            export_value(bill.get("amount"), "amount"),
            export_value(bill.get("main_category"), "main_category"),
            export_value(bill.get("sub_category"), "sub_category"),
            export_account_name(
                bill.get("source_account_id"),
                "source_account",
                &bundle.account_names,
            ),
            export_account_name(
                bill.get("destination_account_id"),
                "destination_account",
                &bundle.account_names,
            ),
            export_value(bill.get("counterparty"), "counterparty"),
            export_value(bill.get("payment_method"), "payment_method"),
            export_value(bill.get("description"), "description"),
            bundle
                .tag_names_by_bill
                .get(&bill_id)
                .map(|tags| tags.join("|"))
                .map(|tags| serialize_optional_export_cell("tags", Some(tags)))
                .unwrap_or_default(),
            export_value(bill.get("comment"), "comment"),
            export_value(bill.get("created_at"), "created_at"),
            export_value(bill.get("updated_at"), "updated_at"),
        ])?;
    }
    let bytes = writer.into_inner().map_err(|error| error.into_error())?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn export_value(value: Option<&Value>, key: &str) -> String {
    match value {
        Some(Value::String(text)) => serialize_optional_export_cell(key, Some(text)),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Null) | None => String::new(),
        Some(value) => serialize_optional_export_cell(key, Some(value.to_string())),
    }
}

fn export_account_name(
    value: Option<&Value>,
    key: &str,
    account_names: &BTreeMap<i64, String>,
) -> String {
    let account_id = value_i64(value).unwrap_or_default();
    account_names
        .get(&account_id)
        .map(|value| serialize_optional_export_cell(key, Some(value)))
        .unwrap_or_default()
}

fn value_i64(value: Option<&Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.parse::<i64>().ok()))
    })
}

fn user_data_clear_all_audit_details(counts: &BTreeMap<String, i64>, auth_mode: &str) -> Value {
    let mut details = Map::new();
    for (key, value) in counts {
        details.insert(key.clone(), json!(value));
    }
    details.insert("auth_mode".to_string(), json!(auth_mode));
    Value::Object(details)
}

fn sensitive_auth_missing_response() -> Response {
    auth_rest_error_response(AuthRestError::new(
        400,
        "Bad Request",
        "Current password or stepUpToken is required",
    ))
}

fn sensitive_auth_invalid_response() -> Response {
    auth_rest_error_response(AuthRestError::new(
        401,
        "Invalid credentials",
        "Current password is incorrect",
    ))
}

fn two_factor_already_enabled_response() -> Response {
    auth_rest_error_response(AuthRestError::new(
        400,
        "Bad Request",
        "Two-factor authentication is already enabled",
    ))
}

