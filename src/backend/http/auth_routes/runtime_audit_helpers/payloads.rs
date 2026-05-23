// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

fn session_payload(session: TokenSessionRow, current_session_id: Option<i64>) -> Value {
    let is_current = current_session_id == Some(session.id);
    let last_activity_at = session.last_activity_at;
    let created_at = session.created_at;
    let last_seen_source = if last_activity_at.is_empty() {
        created_at.as_str()
    } else {
        last_activity_at.as_str()
    };

    json!({
        "tokenId": session.id.to_string(),
        "tokenType": infer_token_type_from_user_agent(&session.user_agent),
        "userAgent": session.user_agent,
        "deviceName": parse_user_agent_device_name(&session.user_agent),
        "ipAddress": session.ip_address,
        "createdAt": created_at,
        "expiresAt": session.expires_at,
        "lastActivityAt": last_activity_at,
        "lastSeen": datetime_to_unix_millis(last_seen_source),
        "isCurrent": is_current,
        "isCurrentToken": is_current
    })
}

fn user_profile_payload(user: &AuthUserProfileRow) -> Value {
    let nickname = if user.nickname.is_empty() {
        user.username.clone()
    } else {
        user.nickname.clone()
    };
    let mut keyword_source = Map::new();
    if let Some(value) = user.investment_platform_keywords.as_deref() {
        keyword_source.insert(
            "investment_platform_keywords".to_string(),
            Value::String(value.to_string()),
        );
    }
    if let Some(value) = user.investment_product_keywords.as_deref() {
        keyword_source.insert(
            "investment_product_keywords".to_string(),
            Value::String(value.to_string()),
        );
    }
    if let Some(value) = user.investment_exclude_keywords.as_deref() {
        keyword_source.insert(
            "investment_exclude_keywords".to_string(),
            Value::String(value.to_string()),
        );
    }
    let investment_settings = build_user_investment_keyword_settings(Some(&keyword_source));
    json!({
        "username": user.username,
        "email": user.email,
        "nickname": nickname,
        "avatar": user.avatar,
        "avatarProvider": "internal",
        "defaultAccountId": optional_id_string(user.default_account_id),
        "transactionEditScope": user.transaction_edit_scope,
        "language": user.language,
        "defaultCurrency": user.default_currency,
        "firstDayOfWeek": user.first_day_of_week,
        "fiscalYearStart": user.fiscal_year_start,
        "calendarDisplayType": user.calendar_display_type,
        "dateDisplayType": user.date_display_type,
        "longDateFormat": user.long_date_format,
        "shortDateFormat": user.short_date_format,
        "longTimeFormat": user.long_time_format,
        "shortTimeFormat": user.short_time_format,
        "fiscalYearFormat": user.fiscal_year_format,
        "currencyDisplayType": user.currency_display_type,
        "numeralSystem": user.numeral_system,
        "decimalSeparator": user.decimal_separator,
        "digitGroupingSymbol": user.digit_grouping_symbol,
        "digitGrouping": user.digit_grouping,
        "coordinateDisplayType": user.coordinate_display_type,
        "expenseAmountColor": user.expense_amount_color,
        "incomeAmountColor": user.income_amount_color,
        "cashAccountId": optional_id_string(user.cash_account_id),
        "cashTransferCategoryId": optional_id_string(user.cash_transfer_category_id),
        "importLearningEnabled": user.import_learning_enabled,
        "investmentPlatformKeywords": investment_settings["platform_keywords"].clone(),
        "investmentProductKeywords": investment_settings["product_keywords"].clone(),
        "investmentExcludeKeywords": investment_settings["exclude_keywords"].clone(),
        "emailVerified": user.email_verified,
    })
}

fn application_cloud_settings_payload(settings: Vec<ApplicationCloudSettingRow>) -> Value {
    Value::Array(
        settings
            .into_iter()
            .map(|setting| {
                json!({
                    "settingKey": setting.setting_key,
                    "settingValue": setting.setting_value,
                })
            })
            .collect(),
    )
}

struct ExternalAuthInfo {
    external_auth_category: String,
    external_auth_type: String,
    linked: bool,
    external_username: String,
    created_at: i64,
}

impl ExternalAuthInfo {
    fn linked(row: ExternalAuthRow) -> Self {
        Self {
            external_auth_category: row.external_auth_category,
            external_auth_type: row.external_auth_type,
            linked: true,
            external_username: row.external_username,
            created_at: datetime_to_unix_millis(&row.created_at),
        }
    }
}

fn external_auths_payload(auths: Vec<ExternalAuthInfo>) -> Value {
    Value::Array(
        auths
            .into_iter()
            .map(|auth| {
                json!({
                    "externalAuthCategory": auth.external_auth_category,
                    "externalAuthType": auth.external_auth_type,
                    "linked": auth.linked,
                    "externalUsername": auth.external_username,
                    "createdAt": auth.created_at,
                })
            })
            .collect(),
    )
}

fn optional_id_string(value: Option<i64>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn datetime_to_unix_millis(value: &str) -> i64 {
    let value = value.trim();
    if value.is_empty() {
        return 0;
    }
    let parsed = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f"));
    parsed
        .ok()
        .and_then(|datetime| Local.from_local_datetime(&datetime).single())
        .map(|datetime| datetime.timestamp_millis())
        .unwrap_or(0)
}
