fn request_body_object(body: &[u8]) -> Map<String, Value> {
    let parsed = serde_json::from_slice::<Value>(body).ok();
    json_object_or_empty(parsed.as_ref())
}

fn validated_profile_updates(
    connection: &rusqlite::Connection,
    user_id: UserId,
    current_user: &AuthUserProfileRow,
    body: &Map<String, Value>,
) -> Result<Vec<AuthUserProfileUpdate>, AuthRestError> {
    if body.contains_key("avatar") {
        return Err(AuthRestError::new(
            400,
            "Bad Request",
            "Avatar must be updated via /api/profile/avatar",
        ));
    }
    let updates = profile_updates_from_body(body)?;
    validate_profile_email_update(connection, user_id, current_user, &updates)?;
    validate_profile_reference_ids(connection, user_id, &updates)?;
    Ok(updates)
}

fn validate_profile_email_update(
    connection: &rusqlite::Connection,
    user_id: UserId,
    current_user: &AuthUserProfileRow,
    updates: &[AuthUserProfileUpdate],
) -> Result<(), AuthRestError> {
    let Some(new_email) = updates.iter().find_map(|update| match update {
        AuthUserProfileUpdate::Email(value) => Some(value),
        _ => None,
    }) else {
        return Ok(());
    };
    if !valid_email_address(new_email) {
        return Err(AuthRestError::new(
            400,
            "Bad Request",
            "Invalid email address",
        ));
    }
    if new_email == &current_user.email {
        return Ok(());
    }
    match auth_email_exists_for_other_user(connection, user_id, new_email) {
        Ok(false) => Ok(()),
        Ok(true) => Err(AuthRestError::new(
            409,
            "Email exists",
            "Email already exists",
        )),
        Err(_) => Err(AuthRestError::new(
            500,
            "Internal Server Error",
            "Rust auth token runtime DB error",
        )),
    }
}

fn validate_profile_reference_ids(
    connection: &rusqlite::Connection,
    user_id: UserId,
    updates: &[AuthUserProfileUpdate],
) -> Result<(), AuthRestError> {
    for update in updates {
        match update {
            AuthUserProfileUpdate::DefaultAccountId(Some(account_id)) => {
                validate_profile_account_id(connection, user_id, *account_id, "defaultAccountId")?;
            }
            AuthUserProfileUpdate::CashAccountId(Some(account_id)) => {
                validate_profile_account_id(connection, user_id, *account_id, "cashAccountId")?;
            }
            AuthUserProfileUpdate::CashTransferCategoryId(Some(category_id)) => {
                validate_profile_category_id(
                    connection,
                    user_id,
                    *category_id,
                    "cashTransferCategoryId",
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_profile_account_id(
    connection: &rusqlite::Connection,
    user_id: UserId,
    account_id: i64,
    field_name: &str,
) -> Result<(), AuthRestError> {
    match auth_account_belongs_to_user(connection, user_id, account_id) {
        Ok(true) => Ok(()),
        Ok(false) => Err(AuthRestError::new(
            400,
            "Bad Request",
            format!("{field_name} is invalid"),
        )),
        Err(_) => Err(AuthRestError::new(
            500,
            "Internal Server Error",
            "Rust auth token runtime DB error",
        )),
    }
}

fn validate_profile_category_id(
    connection: &rusqlite::Connection,
    user_id: UserId,
    category_id: i64,
    field_name: &str,
) -> Result<(), AuthRestError> {
    match auth_category_belongs_to_user(connection, user_id, category_id) {
        Ok(true) => Ok(()),
        Ok(false) => Err(AuthRestError::new(
            400,
            "Bad Request",
            format!("{field_name} is invalid"),
        )),
        Err(_) => Err(AuthRestError::new(
            500,
            "Internal Server Error",
            "Rust auth token runtime DB error",
        )),
    }
}

fn valid_email_address(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.chars().any(char::is_whitespace) || value.matches('@').count() != 1
    {
        return false;
    }
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && domain.contains('.')
        && !domain.contains("..")
        && domain
            .split('.')
            .all(|label| !label.is_empty() && !label.starts_with('-') && !label.ends_with('-'))
}

fn profile_email_changed(updates: &[AuthUserProfileUpdate], current_email: &str) -> Option<String> {
    updates.iter().find_map(|update| match update {
        AuthUserProfileUpdate::Email(value) if value != current_email => Some(value.clone()),
        _ => None,
    })
}

fn profile_updates_from_body(
    body: &Map<String, Value>,
) -> Result<Vec<AuthUserProfileUpdate>, AuthRestError> {
    let mut updates = Vec::new();
    if let Some(value) = body.get("nickname") {
        updates.push(AuthUserProfileUpdate::Nickname(profile_string(
            value, "nickname",
        )?));
    }
    if let Some(value) = body.get("email") {
        updates.push(AuthUserProfileUpdate::Email(
            profile_string(value, "email")?.trim().to_string(),
        ));
    }
    if let Some(value) = body.get("language") {
        updates.push(AuthUserProfileUpdate::Language(profile_string(
            value, "language",
        )?));
    }
    if let Some(value) = body.get("defaultCurrency") {
        updates.push(AuthUserProfileUpdate::DefaultCurrency(profile_string(
            value,
            "defaultCurrency",
        )?));
    }
    if let Some(value) = body.get("firstDayOfWeek") {
        updates.push(AuthUserProfileUpdate::FirstDayOfWeek(profile_i64(
            value,
            "firstDayOfWeek",
        )?));
    }
    if let Some(value) = body.get("defaultAccountId") {
        updates.push(AuthUserProfileUpdate::DefaultAccountId(
            optional_profile_id(value, "defaultAccountId")?,
        ));
    }
    if let Some(value) = body.get("transactionEditScope") {
        updates.push(AuthUserProfileUpdate::TransactionEditScope(profile_i64(
            value,
            "transactionEditScope",
        )?));
    }
    if let Some(value) = body.get("fiscalYearStart") {
        updates.push(AuthUserProfileUpdate::FiscalYearStart(profile_i64(
            value,
            "fiscalYearStart",
        )?));
    }
    if let Some(value) = body.get("calendarDisplayType") {
        updates.push(AuthUserProfileUpdate::CalendarDisplayType(profile_i64(
            value,
            "calendarDisplayType",
        )?));
    }
    if let Some(value) = body.get("dateDisplayType") {
        updates.push(AuthUserProfileUpdate::DateDisplayType(profile_i64(
            value,
            "dateDisplayType",
        )?));
    }
    if let Some(value) = body.get("longDateFormat") {
        updates.push(AuthUserProfileUpdate::LongDateFormat(profile_i64(
            value,
            "longDateFormat",
        )?));
    }
    if let Some(value) = body.get("shortDateFormat") {
        updates.push(AuthUserProfileUpdate::ShortDateFormat(profile_i64(
            value,
            "shortDateFormat",
        )?));
    }
    if let Some(value) = body.get("longTimeFormat") {
        updates.push(AuthUserProfileUpdate::LongTimeFormat(profile_i64(
            value,
            "longTimeFormat",
        )?));
    }
    if let Some(value) = body.get("shortTimeFormat") {
        updates.push(AuthUserProfileUpdate::ShortTimeFormat(profile_i64(
            value,
            "shortTimeFormat",
        )?));
    }
    if let Some(value) = body.get("fiscalYearFormat") {
        updates.push(AuthUserProfileUpdate::FiscalYearFormat(profile_i64(
            value,
            "fiscalYearFormat",
        )?));
    }
    if let Some(value) = body.get("currencyDisplayType") {
        updates.push(AuthUserProfileUpdate::CurrencyDisplayType(profile_i64(
            value,
            "currencyDisplayType",
        )?));
    }
    if let Some(value) = body.get("numeralSystem") {
        updates.push(AuthUserProfileUpdate::NumeralSystem(profile_i64(
            value,
            "numeralSystem",
        )?));
    }
    if let Some(value) = body.get("decimalSeparator") {
        updates.push(AuthUserProfileUpdate::DecimalSeparator(profile_i64(
            value,
            "decimalSeparator",
        )?));
    }
    if let Some(value) = body.get("digitGroupingSymbol") {
        updates.push(AuthUserProfileUpdate::DigitGroupingSymbol(profile_i64(
            value,
            "digitGroupingSymbol",
        )?));
    }
    if let Some(value) = body.get("digitGrouping") {
        updates.push(AuthUserProfileUpdate::DigitGrouping(profile_i64(
            value,
            "digitGrouping",
        )?));
    }
    if let Some(value) = body.get("coordinateDisplayType") {
        updates.push(AuthUserProfileUpdate::CoordinateDisplayType(profile_i64(
            value,
            "coordinateDisplayType",
        )?));
    }
    if let Some(value) = body.get("expenseAmountColor") {
        updates.push(AuthUserProfileUpdate::ExpenseAmountColor(profile_i64(
            value,
            "expenseAmountColor",
        )?));
    }
    if let Some(value) = body.get("incomeAmountColor") {
        updates.push(AuthUserProfileUpdate::IncomeAmountColor(profile_i64(
            value,
            "incomeAmountColor",
        )?));
    }
    if let Some(value) = body.get("cashAccountId") {
        updates.push(AuthUserProfileUpdate::CashAccountId(optional_profile_id(
            value,
            "cashAccountId",
        )?));
    }
    if let Some(value) = body.get("cashTransferCategoryId") {
        updates.push(AuthUserProfileUpdate::CashTransferCategoryId(
            optional_profile_id(value, "cashTransferCategoryId")?,
        ));
    }
    if let Some(value) = body.get("importLearningEnabled") {
        updates.push(AuthUserProfileUpdate::ImportLearningEnabled(profile_bool(
            value,
            "importLearningEnabled",
        )?));
    }
    if let Some(value) = body.get("investmentPlatformKeywords") {
        updates.push(AuthUserProfileUpdate::InvestmentPlatformKeywords(
            serialize_keyword_list(Some(value)),
        ));
    }
    if let Some(value) = body.get("investmentProductKeywords") {
        updates.push(AuthUserProfileUpdate::InvestmentProductKeywords(
            serialize_keyword_list(Some(value)),
        ));
    }
    if let Some(value) = body.get("investmentExcludeKeywords") {
        updates.push(AuthUserProfileUpdate::InvestmentExcludeKeywords(
            serialize_keyword_list(Some(value)),
        ));
    }
    Ok(updates)
}

fn profile_string(value: &Value, field_name: &str) -> Result<String, AuthRestError> {
    value
        .as_str()
        .map(ToString::to_string)
        .ok_or_else(|| AuthRestError::new(400, "Bad Request", format!("{field_name} is invalid")))
}

fn profile_i64(value: &Value, field_name: &str) -> Result<i64, AuthRestError> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| {
            value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .and_then(|value| value.parse().ok())
        })
        .ok_or_else(|| AuthRestError::new(400, "Bad Request", format!("{field_name} is invalid")))
}

fn profile_bool(value: &Value, field_name: &str) -> Result<bool, AuthRestError> {
    if let Some(value) = value.as_bool() {
        return Ok(value);
    }
    if let Some(value) = value.as_i64() {
        return match value {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(AuthRestError::new(
                400,
                "Bad Request",
                format!("{field_name} is invalid"),
            )),
        };
    }
    if let Some(value) = value.as_str() {
        return match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err(AuthRestError::new(
                400,
                "Bad Request",
                format!("{field_name} is invalid"),
            )),
        };
    }
    Err(AuthRestError::new(
        400,
        "Bad Request",
        format!("{field_name} is invalid"),
    ))
}

fn optional_profile_id(value: &Value, field_name: &str) -> Result<Option<i64>, AuthRestError> {
    if value.is_null() {
        return Ok(None);
    }
    if value.as_str().is_some_and(|value| value.trim().is_empty()) {
        return Ok(None);
    }
    let parsed = value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_str().and_then(|value| value.trim().parse().ok()))
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            AuthRestError::new(400, "Bad Request", format!("{field_name} is invalid"))
        })?;
    Ok(Some(parsed))
}

fn avatar_data_url_from_multipart(headers: &HeaderMap, body: &[u8]) -> RouteResult<String> {
    let content_type = header_value(headers, header::CONTENT_TYPE.as_str());
    let boundary = multipart_boundary(&content_type).ok_or_else(|| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Avatar file is required",
        )))
    })?;
    let parts = multipart_parts(body, boundary.as_bytes());
    for part in parts {
        let Some((raw_headers, raw_body)) = split_multipart_part(part) else {
            continue;
        };
        let header_text = String::from_utf8_lossy(raw_headers);
        if !header_text.contains("name=\"avatar\"") {
            continue;
        }
        let payload = trim_trailing_newline(raw_body);
        if payload.is_empty() {
            return Err(Box::new(auth_rest_error_response(AuthRestError::new(
                400,
                "Bad Request",
                "Avatar file is empty",
            ))));
        }
        return avatar_data_url_from_payload(payload, multipart_part_content_type(&header_text));
    }
    Err(Box::new(auth_rest_error_response(AuthRestError::new(
        400,
        "Bad Request",
        "Avatar file is required",
    ))))
}

fn avatar_data_url_from_payload(
    payload: &[u8],
    declared_mime_type: Option<String>,
) -> RouteResult<String> {
    if payload.len() > MAX_AVATAR_BYTES {
        return Err(Box::new(auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Avatar file is too large",
        ))));
    }
    let Some(detected_mime_type) = detect_avatar_mime_type(payload) else {
        return Err(Box::new(auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Unsupported avatar file type",
        ))));
    };
    if let Some(declared_mime_type) = declared_mime_type {
        if !declared_mime_type.eq_ignore_ascii_case(detected_mime_type) {
            return Err(Box::new(auth_rest_error_response(AuthRestError::new(
                400,
                "Bad Request",
                "Avatar MIME type does not match file content",
            ))));
        }
    }
    Ok(format!(
        "data:{detected_mime_type};base64,{}",
        general_purpose::STANDARD.encode(payload)
    ))
}

fn detect_avatar_mime_type(payload: &[u8]) -> Option<&'static str> {
    if payload.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("image/png");
    }
    if payload.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if payload.starts_with(b"GIF87a") || payload.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if payload.len() >= 12 && payload.starts_with(b"RIFF") && &payload[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

fn multipart_boundary(content_type: &str) -> Option<String> {
    content_type.split(';').find_map(|segment| {
        let segment = segment.trim();
        let value = segment.strip_prefix("boundary=")?;
        Some(value.trim_matches('"').to_string())
    })
}

fn multipart_parts<'a>(body: &'a [u8], boundary: &[u8]) -> Vec<&'a [u8]> {
    let delimiter = [b"--".as_slice(), boundary].concat();
    let mut parts = Vec::new();
    let mut search_start = 0;
    while let Some(boundary_start) = find_bytes(&body[search_start..], &delimiter) {
        let part_start = search_start + boundary_start + delimiter.len();
        if body.get(part_start..part_start + 2) == Some(b"--") {
            break;
        }
        let part_start = if body.get(part_start..part_start + 2) == Some(b"\r\n") {
            part_start + 2
        } else if body.get(part_start..part_start + 1) == Some(b"\n") {
            part_start + 1
        } else {
            part_start
        };
        let Some(next_boundary) = find_bytes(&body[part_start..], &delimiter) else {
            break;
        };
        let part_end = part_start + next_boundary;
        parts.push(&body[part_start..part_end]);
        search_start = part_end;
    }
    parts
}

fn split_multipart_part(part: &[u8]) -> Option<(&[u8], &[u8])> {
    if let Some(index) = find_bytes(part, b"\r\n\r\n") {
        return Some((&part[..index], &part[index + 4..]));
    }
    find_bytes(part, b"\n\n").map(|index| (&part[..index], &part[index + 2..]))
}

fn trim_trailing_newline(mut value: &[u8]) -> &[u8] {
    if value.ends_with(b"\r\n") {
        value = &value[..value.len().saturating_sub(2)];
    } else if value.ends_with(b"\n") {
        value = &value[..value.len().saturating_sub(1)];
    }
    value
}

fn multipart_part_content_type(header_text: &str) -> Option<String> {
    header_text.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case("content-type") {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
        None
    })
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn validate_application_cloud_setting(
    setting: &Value,
) -> Result<ApplicationCloudSettingDraft, AuthRestError> {
    let setting_key = setting
        .get("settingKey")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let Some(setting_value) = setting.get("settingValue").and_then(Value::as_str) else {
        return Err(AuthRestError::new(
            400,
            "Bad Request",
            format!("Invalid setting value for {setting_key}"),
        ));
    };
    if setting_key.is_empty() {
        return Err(AuthRestError::new(
            400,
            "Bad Request",
            "settingKey is required",
        ));
    }
    match application_cloud_setting_type(&setting_key) {
        Some("string") => {}
        Some("number") => {
            if setting_value.parse::<f64>().is_err() {
                return Err(AuthRestError::new(
                    400,
                    "Bad Request",
                    format!("Invalid number value for {setting_key}"),
                ));
            }
        }
        Some("boolean") => {
            if !matches!(setting_value, "true" | "false") {
                return Err(AuthRestError::new(
                    400,
                    "Bad Request",
                    format!("Invalid boolean value for {setting_key}"),
                ));
            }
        }
        Some("string_boolean_map") => {
            let parsed = serde_json::from_str::<Value>(setting_value).map_err(|_| {
                AuthRestError::new(
                    400,
                    "Bad Request",
                    format!("Invalid JSON value for {setting_key}"),
                )
            })?;
            let Some(object) = parsed.as_object() else {
                return Err(AuthRestError::new(
                    400,
                    "Bad Request",
                    format!("Invalid map value for {setting_key}"),
                ));
            };
            if object
                .iter()
                .any(|(map_key, map_value)| map_key.is_empty() || !map_value.is_boolean())
            {
                return Err(AuthRestError::new(
                    400,
                    "Bad Request",
                    format!("Invalid map value for {setting_key}"),
                ));
            }
        }
        Some(_) => {
            return Err(AuthRestError::new(
                400,
                "Bad Request",
                format!("Unsupported setting type for {setting_key}"),
            ));
        }
        None => {
            return Err(AuthRestError::new(
                400,
                "Bad Request",
                format!("Unsupported setting key: {setting_key}"),
            ));
        }
    }
    Ok(ApplicationCloudSettingDraft {
        setting_key,
        setting_value: setting_value.to_string(),
    })
}

fn application_cloud_setting_type(setting_key: &str) -> Option<&'static str> {
    match setting_key {
        "showAccountBalance"
        | "showAmountInHomePage"
        | "showTotalAmountInTransactionListPage"
        | "showTagInTransactionListPage"
        | "autoGetCurrentGeoLocation"
        | "alwaysShowTransactionPicturesInMobileTransactionEditPage" => Some("boolean"),
        "timezoneUsedForStatisticsInHomePage"
        | "itemsCountInTransactionListPage"
        | "currencySortByInExchangeRatesPage"
        | "statistics.defaultChartDataType"
        | "statistics.defaultTimezoneType"
        | "statistics.defaultSortingType"
        | "statistics.defaultCategoricalChartType"
        | "statistics.defaultCategoricalChartDataRangeType"
        | "statistics.defaultTrendChartType"
        | "statistics.defaultTrendChartDataRangeType"
        | "statistics.defaultAssetTrendsChartType"
        | "statistics.defaultAssetTrendsChartDataRangeType" => Some("number"),
        "overviewAccountFilterInHomePage"
        | "overviewTransactionCategoryFilterInHomePage"
        | "totalAmountExcludeAccountIds"
        | "statistics.defaultAccountFilter"
        | "statistics.defaultTransactionCategoryFilter" => Some("string_boolean_map"),
        "autoSaveTransactionDraft" => Some("string"),
        _ => None,
    }
}

fn register_preset_categories_from_body(body: &Map<String, Value>) -> Vec<RegisterPresetCategory> {
    let Some(categories) = body.get("categories").and_then(Value::as_array) else {
        return Vec::new();
    };
    categories
        .iter()
        .filter_map(|item| {
            let object = item.as_object()?;
            let name = object
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string();
            if name.is_empty() {
                return None;
            }
            let sub_categories = object
                .get("subCategories")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|sub_item| {
                    let sub_object = sub_item.as_object()?;
                    let name = sub_object
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    if name.is_empty() {
                        return None;
                    }
                    Some(RegisterPresetSubCategory {
                        name,
                        icon: sub_object
                            .get("icon")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        color: sub_object
                            .get("color")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    })
                })
                .collect();
            Some(RegisterPresetCategory {
                name,
                type_code: object.get("type").and_then(Value::as_i64).unwrap_or(3),
                icon: object
                    .get("icon")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                color: object
                    .get("color")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                sub_categories,
            })
        })
        .collect()
}

fn parse_expires_in_seconds(body: &Map<String, Value>) -> RouteResult<i64> {
    let Some(value) = body.get("expiresInSeconds") else {
        return Ok(0);
    };
    if value.is_null() {
        return Ok(0);
    }
    match value {
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                Ok(value)
            } else if let Some(value) = number.as_u64() {
                i64::try_from(value).map_err(|_| invalid_expires_response())
            } else {
                number
                    .as_f64()
                    .map(|value| value as i64)
                    .ok_or_else(invalid_expires_response)
            }
        }
        Value::String(value) => value.parse::<i64>().map_err(|_| invalid_expires_response()),
        Value::Bool(value) => Ok(if *value { 1 } else { 0 }),
        _ => Err(invalid_expires_response()),
    }
}

fn invalid_expires_response() -> Box<Response> {
    Box::new(auth_rest_error_response(AuthRestError::invalid_request(
        "expiresInSeconds must be a valid integer",
    )))
}

