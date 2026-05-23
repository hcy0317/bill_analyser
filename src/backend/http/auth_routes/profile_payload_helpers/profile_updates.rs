// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

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
