// 中文说明：把前端账户 DTO 转成 repository payload，强制余额字段使用整数 cents。
fn frontend_account_to_backend(payload: &Value) -> Result<Map<String, Value>, String> {
    let Some(object) = payload.as_object() else {
        return Err("Account payload must be an object".to_string());
    };
    let balance_cents = account_balance_cents_value(object)?;
    let hidden = object
        .get("hidden")
        .map(value_truthy)
        .unwrap_or_else(|| !object.get("visible").map(value_truthy).unwrap_or(true));

    let mut result = Map::new();
    result.insert(
        "name".to_string(),
        Value::String(string_or_default(object.get("name"), "")),
    );
    result.insert(
        "parent_id".to_string(),
        Value::Number(Number::from(
            object
                .get("parentId")
                .or_else(|| object.get("parent_id"))
                .and_then(value_as_i64)
                .unwrap_or(0),
        )),
    );
    result.insert(
        "category".to_string(),
        object.get("category").cloned().unwrap_or(Value::Null),
    );
    result.insert(
        "type".to_string(),
        object
            .get("type")
            .cloned()
            .unwrap_or_else(|| Value::Number(Number::from(0))),
    );
    result.insert(
        "icon".to_string(),
        Value::String(string_or_default(object.get("icon"), "")),
    );
    result.insert(
        "color".to_string(),
        Value::String(string_or_default(object.get("color"), "")),
    );
    result.insert(
        "currency".to_string(),
        Value::String(string_or_default(object.get("currency"), "CNY")),
    );
    result.insert(
        "balance_cents".to_string(),
        Value::Number(Number::from(balance_cents)),
    );
    result.insert(
        "initial_balance_cents".to_string(),
        Value::Number(Number::from(balance_cents)),
    );
    result.insert(
        "comment".to_string(),
        Value::String(string_or_default(object.get("comment"), "")),
    );
    result.insert(
        "display_order".to_string(),
        Value::Number(Number::from(
            object
                .get("displayOrder")
                .or_else(|| object.get("display_order"))
                .and_then(value_as_i64)
                .unwrap_or(0),
        )),
    );
    result.insert("hidden".to_string(), Value::Bool(hidden));
    if let Some(Value::Array(sub_accounts)) = object.get("subAccounts") {
        let converted_sub_accounts = sub_accounts
            .iter()
            .map(frontend_account_to_backend)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(Value::Object)
            .collect();
        result.insert(
            "subAccounts".to_string(),
            Value::Array(converted_sub_accounts),
        );
    }
    if let Some(statement_date) = object
        .get("creditCardStatementDate")
        .or_else(|| object.get("credit_card_statement_date"))
    {
        result.insert(
            "credit_card_statement_date".to_string(),
            statement_date.clone(),
        );
    }
    Ok(result)
}

// 中文说明：读取账户余额/期初余额 cents 字段，兼容当前 camelCase 与旧 snake_case 输入。
fn account_balance_cents_value(object: &Map<String, Value>) -> Result<i64, String> {
    for field in [
        "balanceCents",
        "balance_cents",
        "initialBalanceCents",
        "initial_balance_cents",
    ] {
        if let Some(value) = object.get(field) {
            return strict_account_cents_value(value, field);
        }
    }
    Ok(0)
}

fn strict_account_cents_value(value: &Value, field: &str) -> Result<i64, String> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .ok_or_else(|| format!("{field} must be integer cents")),
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                Err(format!("{field} must be integer cents"))
            } else {
                trimmed
                    .parse::<i64>()
                    .map_err(|_| format!("{field} must be integer cents"))
            }
        }
        Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => {
            Err(format!("{field} must be integer cents"))
        }
    }
}
