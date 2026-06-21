fn bool_as_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn payload_scalar_text(value: &Value) -> DbResult<String> {
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Number(number) => Ok(number.to_string()),
        Value::Bool(flag) => Ok(flag.to_string()),
        Value::Null => Ok(String::new()),
        Value::Array(_) | Value::Object(_) => Err(DbError::InvalidOperation(
            "text field must be scalar".to_string(),
        )),
    }
}

fn payload_bool_value(value: &Value) -> DbResult<bool> {
    match value {
        Value::Bool(flag) => Ok(*flag),
        Value::Number(number) => number.as_i64().map(|value| value != 0).ok_or_else(|| {
            DbError::InvalidOperation("boolean field must be an integer".to_string())
        }),
        Value::String(text) => {
            let normalized = text.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "true" | "1" | "yes" | "on" => Ok(true),
                "false" | "0" | "no" | "off" | "" => Ok(false),
                _ => Err(DbError::InvalidOperation(
                    "boolean field must be truthy or falsy".to_string(),
                )),
            }
        }
        Value::Null | Value::Array(_) | Value::Object(_) => Err(DbError::InvalidOperation(
            "boolean field must be truthy or falsy".to_string(),
        )),
    }
}

fn required_postgres_i64(value: &Value, field: &str) -> DbResult<i64> {
    int_value(Some(value))
        .ok_or_else(|| DbError::InvalidOperation(format!("{field} must be an integer-like value")))
}

fn required_postgres_rule_expression(value: Option<&Value>) -> DbResult<String> {
    match value {
        Some(Value::Null) | None => Err(DbError::InvalidOperation(
            "rule_expression is required".to_string(),
        )),
        Some(value) => {
            let expression = payload_scalar_text(value)?;
            if expression.trim().is_empty() {
                Err(DbError::InvalidOperation(
                    "rule_expression is required".to_string(),
                ))
            } else {
                Ok(expression)
            }
        }
    }
}

fn rule_expression_json(expression: &str, regex_enabled: bool) -> Value {
    serde_json::json!({
        "expression": expression,
        "regex_enabled": regex_enabled,
    })
}

fn rule_expression_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Object(object) => object
            .get("expression")
            .or_else(|| object.get("rule_expression"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| contains_any_expression(object))
            .unwrap_or_default(),
        Value::Null => String::new(),
        Value::Number(_) | Value::Bool(_) | Value::Array(_) => value.to_string(),
    }
}

fn rule_expression_regex_enabled(value: &Value) -> bool {
    value
        .get("regex_enabled")
        .map(value_truthy)
        .unwrap_or(false)
}

fn contains_any_expression(object: &Map<String, Value>) -> Option<String> {
    let operator = object
        .get("operator")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !operator.eq_ignore_ascii_case("contains_any") {
        return None;
    }
    let values = object
        .get("values")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|value| match value {
            Value::String(text) => Some(text.trim().to_string()),
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(flag) => Some(flag.to_string()),
            Value::Null | Value::Array(_) | Value::Object(_) => None,
        })
        .filter(|value| !value.is_empty())
        .map(|value| escape_rule_expression_term(&value))
        .collect::<Vec<_>>();
    (!values.is_empty()).then(|| format!("OR={{{}}}", values.join(",")))
}

fn json_array(values: &[&str]) -> Value {
    Value::Array(
        values
            .iter()
            .map(|value| Value::String((*value).to_string()))
            .collect(),
    )
}

async fn postgres_category_belongs_to_user(
    pool: &PostgresPool,
    category_id: i64,
    user_id: i64,
) -> DbResult<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM categories WHERE id = $1 AND user_id = $2)",
    )
    .bind(category_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

async fn postgres_account_belongs_to_user(
    pool: &PostgresPool,
    account_id: i64,
    user_id: i64,
) -> DbResult<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = $1 AND user_id = $2)",
    )
    .bind(account_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

async fn count_postgres_account_rules(
    pool: &PostgresPool,
    rule_ids: &BTreeSet<i64>,
    user_id: i64,
) -> DbResult<i64> {
    let mut builder = QueryBuilder::<Postgres>::new(
        "SELECT COUNT(*)::BIGINT AS count FROM account_rules WHERE user_id = ",
    );
    builder.push_bind(user_id);
    builder.push(" AND id IN (");
    let mut separated = builder.separated(", ");
    for rule_id in rule_ids {
        separated.push_bind(rule_id);
    }
    separated.push_unseparated(")");
    let row = builder.build().fetch_one(pool).await?;
    row.try_get("count").map_err(Into::into)
}

fn account_rule_candidate_from_record(record: AccountRuleRecord) -> DbResult<AccountRuleCandidate> {
    Ok(AccountRuleCandidate {
        rule_id: int_value(record.get("id")).unwrap_or_default(),
        account_id: int_value(record.get("account_id")).unwrap_or_default(),
        rule_expression: record
            .get("rule_expression")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        regex_enabled: record.get("regex_enabled").is_some_and(value_truthy),
        enabled: record.get("enabled").map(value_truthy).unwrap_or(true),
        priority: int_value(record.get("priority")).unwrap_or(100),
    })
}

fn optional_strict_minor_units(value: Option<&Value>, field: &str) -> DbResult<Option<i64>> {
    value
        .map(|value| strict_minor_units_value(value, field))
        .transpose()
}

fn strict_minor_units_value(value: &Value, field: &str) -> DbResult<i64> {
    strict_cents_value(value).ok_or_else(|| {
        DbError::InvalidOperation(format!("{field} must be integer cents/minor units"))
    })
}

fn i64_to_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or_else(|_| {
        if value.is_negative() {
            i32::MIN
        } else {
            i32::MAX
        }
    })
}
