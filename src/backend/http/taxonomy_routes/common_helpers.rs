// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

fn category_export_record(category: &CategoryRecord) -> Map<String, Value> {
    let mut result = Map::new();
    for (field, default) in [
        ("type", Value::Number(Number::from(3))),
        ("main_category", Value::String(String::new())),
        ("sub_category", Value::String(String::new())),
        ("priority", Value::Number(Number::from(0))),
        ("keywords", Value::String(String::new())),
        ("description", Value::String(String::new())),
        ("icon", Value::String(String::new())),
        ("color", Value::String(String::new())),
        ("hidden", Value::Bool(false)),
    ] {
        result.insert(
            field.to_string(),
            category.get(field).cloned().unwrap_or(default),
        );
    }
    result
}

fn backend_category_to_frontend(category: &CategoryRecord, parent_id: &str) -> Map<String, Value> {
    let hidden = category.get("hidden").map(value_truthy).unwrap_or(false);
    let sub_category = category_text(category, "sub_category");
    let name = if sub_category.is_empty() {
        category_text(category, "main_category")
    } else {
        sub_category
    };
    let mut result = Map::new();
    result.insert(
        "id".to_string(),
        Value::String(value_string(category.get("id"), "")),
    );
    result.insert("name".to_string(), Value::String(name));
    result.insert("parentId".to_string(), Value::String(parent_id.to_string()));
    result.insert(
        "type".to_string(),
        category
            .get("type")
            .cloned()
            .unwrap_or_else(|| Value::Number(Number::from(0))),
    );
    result.insert(
        "icon".to_string(),
        Value::String(string_or_default(category.get("icon"), "")),
    );
    result.insert(
        "color".to_string(),
        Value::String(string_or_default(category.get("color"), "")),
    );
    result.insert(
        "comment".to_string(),
        Value::String(string_or_default(category.get("description"), "")),
    );
    result.insert(
        "displayOrder".to_string(),
        category
            .get("priority")
            .cloned()
            .unwrap_or_else(|| Value::Number(Number::from(0))),
    );
    result.insert("hidden".to_string(), Value::Bool(hidden));
    result.insert("visible".to_string(), Value::Bool(!hidden));
    result.insert(
        "keywords".to_string(),
        Value::String(string_or_default(category.get("keywords"), "")),
    );
    result
}

fn category_text(category: &CategoryRecord, key: &str) -> String {
    string_or_default(category.get(key), "")
}

fn required_json_body(body: Bytes, missing_message: &'static str) -> RouteResult<Value> {
    let value = parse_json_body(body)?;
    if matches!(value, Value::Null) || value.as_object().is_some_and(Map::is_empty) {
        return Err(Box::new(bad_request(missing_message)));
    }
    Ok(value)
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_json_body(body: Bytes) -> RouteResult<Value> {
    if body.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_slice(&body).map_err(|_| Box::new(bad_request("Invalid JSON")))
}

fn open_runtime(state: &HttpAppState, runtime_label: &'static str) -> RouteResult<SqliteRuntime> {
    state
        .open_sqlite_repository_runtime(runtime_label)
        .map_err(|error| {
            Box::new(error_response(
                status_or_internal(error.http_status_code()),
                error.public_message("Rust taxonomy route runtime DB error"),
            ))
        })
}

fn open_postgres_runtime(
    state: &HttpAppState,
    runtime_label: &'static str,
) -> RouteResult<bill_analyser_db::PostgresRepositoryRuntime> {
    state
        .open_postgres_repository_runtime(runtime_label)
        .map_err(|error| {
            Box::new(error_response(
                status_or_internal(error.http_status_code()),
                error.public_message("Rust taxonomy route PostgreSQL runtime DB error"),
            ))
        })
}

fn user_id_from_headers(headers: &HeaderMap, config: &HttpShellConfig) -> RouteResult<UserId> {
    resolve_user_id_from_headers(headers, config, TRUSTED_USER_SECRET_HEADER).map_err(|error| {
        Box::new(error_response(
            status_or_internal(error.status),
            error.message,
        ))
    })
}

fn db_user_id(user_id: UserId) -> i64 {
    i64::try_from(user_id.get()).unwrap_or(i64::MAX)
}

fn json_response(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

fn success_result(status: StatusCode, result: Value) -> Response {
    json_response(status, json!({ "success": true, "result": result }))
}

fn success_result_with_message(status: StatusCode, result: Value, message: &str) -> Response {
    json_response(
        status,
        json!({ "success": true, "result": result, "message": message }),
    )
}

fn bad_request(message: impl ToString) -> Response {
    error_response(StatusCode::BAD_REQUEST, message)
}

fn not_found(message: impl ToString) -> Response {
    error_response(StatusCode::NOT_FOUND, message)
}

fn db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust taxonomy accounts route runtime DB error",
    )
}

fn tag_db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust taxonomy tags route runtime DB error",
    )
}

fn category_db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust taxonomy categories route runtime DB error",
    )
}

fn category_rule_db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust taxonomy category rules route runtime DB error",
    )
}

fn account_rule_db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust taxonomy account rules route runtime DB error",
    )
}

fn template_db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust taxonomy templates route runtime DB error",
    )
}

fn error_response(status: StatusCode, message: impl ToString) -> Response {
    json_response(
        status,
        json!({ "success": false, "error": message.to_string() }),
    )
}

fn status_or_internal(status: u16) -> StatusCode {
    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_aliases(value: Option<&Value>) -> Vec<String> {
    match value {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(values)) => values
            .iter()
            .map(python_value_text)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        Some(Value::String(text)) => parse_alias_string(text),
        Some(_) => Vec::new(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_alias_string(text: &str) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    if text.starts_with('[') {
        if let Ok(Value::Array(values)) = serde_json::from_str::<Value>(text) {
            return values
                .iter()
                .map(python_value_text)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect();
        }
    }
    text.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn python_value_text(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        Value::String(text) => text.clone(),
        Value::Number(_) | Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn template_type_from_query_body(
    query: &BTreeMap<String, String>,
    body: Option<&Value>,
    default: Option<i64>,
) -> Option<i64> {
    if let Some(value) = query.get("templateType") {
        return parse_template_type_text(value).or(default);
    }
    if let Some(value) = body.and_then(|value| value.get("templateType")) {
        return parse_template_type_text(&python_value_text(value)).or(default);
    }
    default
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_template_type_text(value: &str) -> Option<i64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    value.parse::<i64>().ok()
}

fn template_display_orders_from_body(body: &Value) -> RouteResult<Vec<TemplateDisplayOrder>> {
    let Some(items) = body
        .get("newDisplayOrders")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
    else {
        return Err(Box::new(bad_request("Missing newDisplayOrders")));
    };
    let mut orders = Vec::with_capacity(items.len());
    for item in items {
        let Some(object) = item.as_object() else {
            return Err(Box::new(bad_request(
                "Each item must have id and displayOrder",
            )));
        };
        let Some(template_id) = object.get("id").and_then(parse_python_int) else {
            return Err(Box::new(bad_request(
                "Each item must have id and displayOrder",
            )));
        };
        let Some(display_order) = object.get("displayOrder").and_then(parse_python_int) else {
            return Err(Box::new(bad_request(
                "Each item must have id and displayOrder",
            )));
        };
        orders.push(TemplateDisplayOrder {
            template_id,
            display_order,
        });
    }
    Ok(orders)
}

fn tag_name_is_present(payload: &Value) -> bool {
    payload
        .as_object()
        .and_then(|object| object.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn tag_batch_normalized_name(payload: &Value) -> Option<String> {
    let object = payload.as_object()?;
    let name = value_string(object.get("name"), "");
    let trimmed = name.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_lowercase())
    }
}

fn tag_batch_display_name(payload: &Value) -> String {
    payload
        .as_object()
        .map(|object| value_string(object.get("name"), ""))
        .unwrap_or_default()
}

fn frontend_tag_to_backend(payload: &Value) -> Result<Map<String, Value>, String> {
    let Some(object) = payload.as_object() else {
        return Err("Tag payload must be an object".to_string());
    };
    let mut result = Map::new();

    if let Some(id) = object.get("id") {
        result.insert("id".to_string(), id.clone());
    }
    if let Some(name) = object.get("name") {
        result.insert(
            "name".to_string(),
            Value::String(string_or_default(Some(name), "")),
        );
    }
    if let Some(color) = object.get("color") {
        result.insert(
            "color".to_string(),
            if color.is_null() {
                Value::Null
            } else {
                Value::String(string_or_default(Some(color), "#000000"))
            },
        );
    }
    if let Some(icon) = object.get("icon") {
        result.insert(
            "icon".to_string(),
            if icon.is_null() {
                Value::Null
            } else {
                Value::String(string_or_default(Some(icon), ""))
            },
        );
    }
    if let Some(hidden) = object.get("hidden") {
        result.insert("hidden".to_string(), Value::Bool(value_truthy(hidden)));
    } else if let Some(visible) = object.get("visible") {
        result.insert("hidden".to_string(), Value::Bool(!value_truthy(visible)));
    }
    if let Some(display_order) = object
        .get("displayOrder")
        .or_else(|| object.get("display_order"))
    {
        let Some(display_order) = value_as_i64(display_order) else {
            return Err("displayOrder must be an integer".to_string());
        };
        result.insert(
            "display_order".to_string(),
            Value::Number(Number::from(display_order)),
        );
    }

    Ok(result)
}

fn value_string(value: Option<&Value>, default: &str) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(Value::Null) | None => default.to_string(),
        Some(value @ (Value::Array(_) | Value::Object(_))) => value.to_string(),
    }
}

fn string_or_default(value: Option<&Value>, default: &str) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Null) | None => default.to_string(),
        Some(value) => value.to_string(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_python_int(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        Value::Bool(flag) => Some(i64::from(*flag)),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn invalid_tag_order_value_error(value: Option<&Value>) -> String {
    let reason = match value {
        Some(Value::String(text)) => {
            format!("invalid literal for int() with base 10: '{text}'")
        }
        Some(Value::Null) => {
            "int() argument must be a string, a bytes-like object or a real number, not 'NoneType'"
                .to_string()
        }
        Some(value) => format!("unsupported integer value: {value}"),
        None => "missing value".to_string(),
    };
    format!("Invalid id or displayOrder: {reason}")
}

fn value_as_i64(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| {
        value
            .as_str()
            .and_then(|text| text.trim().parse::<i64>().ok())
    })
}

fn value_as_f64(value: &Value) -> Option<f64> {
    value.as_f64().or_else(|| {
        value
            .as_str()
            .and_then(|text| text.trim().parse::<f64>().ok())
    })
}

fn value_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_i64().unwrap_or_default() != 0,
        Value::String(text) => {
            let trimmed = text.trim();
            !trimmed.is_empty()
                && !trimmed.eq_ignore_ascii_case("false")
                && trimmed != "0"
                && !trimmed.eq_ignore_ascii_case("none")
                && !trimmed.eq_ignore_ascii_case("null")
        }
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
        Value::Null => false,
    }
}

fn yuan_to_cents(value: Option<&Value>) -> i64 {
    let yuan = value.and_then(value_as_f64).unwrap_or_default();
    (yuan * 100.0).round() as i64
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn json_number(value: f64) -> Value {
    Number::from_f64(value).map_or(Value::Null, Value::Number)
}
