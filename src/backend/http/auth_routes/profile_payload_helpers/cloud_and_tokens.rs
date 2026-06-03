// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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
