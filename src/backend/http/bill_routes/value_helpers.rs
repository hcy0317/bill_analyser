// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

fn value_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(text.trim().to_string()),
        Some(Value::Number(number)) => Some(number.to_string()),
        Some(Value::Bool(value)) => Some(value.to_string()),
        _ => None,
    }
    .filter(|value| !value.is_empty())
}

fn non_empty_string(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn record_text(record: &Map<String, Value>, key: &str) -> String {
    value_string(record.get(key)).unwrap_or_default()
}

fn record_i64(record: &Map<String, Value>, key: &str) -> Option<i64> {
    record.get(key).and_then(value_to_i64)
}

fn value_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn positive_record_i64(record: &Map<String, Value>, key: &str) -> Option<i64> {
    record_i64(record, key).filter(|value| *value > 0)
}

fn record_f64(record: &Map<String, Value>, key: &str) -> bill_analyser_db::DbResult<f64> {
    match record.get(key) {
        Some(Value::Number(number)) => number.as_f64().ok_or_else(|| {
            bill_analyser_db::DbError::InvalidOperation(format!("invalid numeric field: {key}"))
        }),
        Some(Value::String(text)) => text.trim().parse::<f64>().map_err(|_| {
            bill_analyser_db::DbError::InvalidOperation(format!("invalid numeric field: {key}"))
        }),
        _ => Err(bill_analyser_db::DbError::InvalidOperation(format!(
            "missing numeric field: {key}"
        ))),
    }
}

fn money_from_record(record: &Map<String, Value>, key: &str) -> bill_analyser_db::DbResult<Money> {
    let value = record_f64(record, key)?;
    Money::from_yuan_str(&finite_float_text(value)).map_err(runtime_error)
}

fn frontend_tag_from_value(value: &Value) -> Option<FrontendTransactionTag> {
    let object = value.as_object()?;
    Some(FrontendTransactionTag {
        id: value_string(object.get("id"))?,
        name: value_string(object.get("name")).unwrap_or_default(),
    })
}

fn date_from_timestamp(value: i64) -> Option<String> {
    let seconds = if value.abs() >= 100_000_000_000 {
        value / 1000
    } else {
        value
    };
    DateTime::from_timestamp(seconds, 0)
        .map(|date| date.with_timezone(&Local).format("%Y-%m-%d").to_string())
}

fn json_number(value: f64) -> Value {
    Number::from_f64(value).map_or(Value::Null, Value::Number)
}

fn is_non_empty_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.trim().is_empty(),
        _ => true,
    }
}

fn finite_float_text(value: f64) -> String {
    let text = value.to_string();
    if value.is_finite() && !text.contains('.') && !text.contains('e') && !text.contains('E') {
        format!("{text}.0")
    } else {
        text
    }
}

fn runtime_error(error: RuntimeError) -> bill_analyser_db::DbError {
    bill_analyser_db::DbError::InvalidOperation(error.to_string())
}

#[cfg(test)]
mod value_helper_tests {
    use super::*;

    #[test]
    fn value_helpers_cover_scalar_parse_and_error_edges() {
        assert_eq!(value_string(Some(&json!(true))), Some("true".to_string()));
        assert_eq!(value_to_i64(&json!("12")), Some(12));
        assert_eq!(value_to_i64(&json!({})), None);

        let mut invalid_record = Map::new();
        invalid_record.insert("amount".to_string(), Value::String("not-a-number".to_string()));
        assert!(record_f64(&invalid_record, "amount").is_err());
        assert!(record_f64(&Map::new(), "amount").is_err());

        assert_eq!(
            frontend_tag_from_value(&json!({"id": true, "name": false}))
                .expect("tag")
                .name,
            "false"
        );
        assert!(!is_non_empty_value(&Value::Null));
        assert!(!is_non_empty_value(&Value::String(" ".to_string())));
        assert!(is_non_empty_value(&json!(1)));

        let error = RuntimeError::new(bill_analyser_core::ErrorCode::InvalidInput, "bad route");
        assert!(runtime_error(error).to_string().contains("bad route"));
    }
}
