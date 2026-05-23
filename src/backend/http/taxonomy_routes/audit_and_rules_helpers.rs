// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

fn verify_sensitive_export_password(
    connection: &Connection,
    user_id: i64,
    password: &str,
) -> rusqlite::Result<bool> {
    let password_hash = connection.query_row(
        "SELECT password_hash FROM users WHERE id = ?1",
        params![user_id],
        |row| row.get::<_, Option<String>>(0),
    )?;
    Ok(password_hash
        .filter(|value| !value.is_empty())
        .is_some_and(|hash| bcrypt::verify(password, &hash).unwrap_or(false)))
}

struct AccountAuditLogDraft {
    operation_type: &'static str,
    target_id: i64,
    details: Value,
    affected_count: i64,
    status: &'static str,
    error_message: Option<String>,
    ip_address: String,
    user_agent: String,
}

fn verify_sensitive_account_operation_password(
    connection: &Connection,
    user_id: i64,
    password: &str,
) -> rusqlite::Result<bool> {
    if password.is_empty() {
        return Ok(false);
    }
    match verify_sensitive_export_password(connection, user_id, password) {
        Ok(true) => return Ok(true),
        Ok(false) | Err(rusqlite::Error::QueryReturnedNoRows) => {}
        Err(error) => return Err(error),
    }

    if let Some(env_password) = std::env::var("BILL_ANALYSER_OPERATION_PASSWORD")
        .ok()
        .filter(|value| !value.is_empty())
    {
        return Ok(password == env_password);
    }

    if !sqlite_table_exists(connection, "app_settings")? {
        return Ok(true);
    }
    let stored_password = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params!["operation_password"],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();
    Ok(
        match stored_password.as_deref().filter(|value| !value.is_empty()) {
            Some(value) => password == value,
            None => true,
        },
    )
}

fn create_account_audit_log_best_effort(connection: &Connection, draft: AccountAuditLogDraft) {
    let now = Utc::now()
        .naive_utc()
        .format("%Y-%m-%dT%H:%M:%S%.6f")
        .to_string();
    let _ = connection.execute(
        r#"
        INSERT INTO audit_logs (
            operation_type, operation_target, target_id, details,
            affected_count, ip_address, user_agent, session_id,
            status, error_message, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
        (
            draft.operation_type,
            "account",
            draft.target_id,
            draft.details.to_string(),
            draft.affected_count,
            draft.ip_address,
            draft.user_agent,
            Option::<String>::None,
            draft.status,
            draft.error_message,
            now,
        ),
    );
}

fn audit_ip_address(headers: &HeaderMap) -> String {
    header_string(headers, "x-forwarded-for")
        .split(',')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            let value = header_string(headers, "x-real-ip");
            if value.is_empty() {
                None
            } else {
                Some(value)
            }
        })
        .unwrap_or_default()
}

fn audit_user_agent(headers: &HeaderMap) -> String {
    header_string(headers, "user-agent")
}

fn header_string(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

fn sqlite_table_exists(connection: &Connection, table_name: &str) -> rusqlite::Result<bool> {
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        params![table_name],
        |row| row.get::<_, i64>(0),
    )? == 1)
}

fn optional_json_body(body: Bytes) -> Option<Value> {
    if body.is_empty() {
        return None;
    }
    serde_json::from_slice(&body).ok()
}

fn template_tag_ids(value: Option<&Value>) -> Vec<Value> {
    let Some(value) = value else {
        return Vec::new();
    };
    if let Some(values) = value.as_array() {
        return values
            .iter()
            .map(|item| Value::String(string_or_default(Some(item), "")))
            .filter(|item| item.as_str().is_some_and(|text| !text.is_empty()))
            .collect();
    }
    string_or_default(Some(value), "")
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| Value::String(item.to_string()))
        .collect()
}

fn recurring_string_or_null(row: &Map<String, Value>, template_type: i64, key: &str) -> Value {
    if template_type != 2 {
        return Value::Null;
    }
    row.get(key).cloned().unwrap_or(Value::Null)
}

fn normalize_template_transaction_type(value: Option<&Value>) -> i64 {
    let text = string_or_default(value, "").to_ascii_lowercase();
    match text.as_str() {
        "2" | "income" | "收入" => 2,
        "4" | "transfer" | "转账" => 4,
        "5" | "investment" | "投资" => 5,
        _ => 3,
    }
}

fn normalize_llm_advanced_settings(raw_value: &str) -> Value {
    let loaded = serde_json::from_str::<Value>(raw_value).unwrap_or_else(|_| json!({}));
    let object = loaded.as_object();
    let mut normalized = Map::new();

    if let Some(reasoning_depth) = object
        .and_then(|settings| settings.get("reasoning_depth"))
        .map(|value| string_or_default(Some(value), "").to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "low" | "medium" | "high"))
    {
        normalized.insert(
            "reasoning_depth".to_string(),
            Value::String(reasoning_depth),
        );
    }
    if let Some(temperature) = object
        .and_then(|settings| settings.get("temperature"))
        .and_then(value_as_f64)
        .filter(|value| (0.0..=2.0).contains(value))
    {
        normalized.insert("temperature".to_string(), json_number(temperature));
    }
    if let Some(max_tokens) = object
        .and_then(|settings| settings.get("max_tokens"))
        .and_then(value_as_i64)
        .filter(|value| (1..=200_000).contains(value))
    {
        normalized.insert(
            "max_tokens".to_string(),
            Value::Number(Number::from(max_tokens)),
        );
    }
    for key in [
        "system_prompt",
        "classification_prompt_template",
        "rule_prompt_template",
    ] {
        if let Some(text) = object
            .and_then(|settings| settings.get(key))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            normalized.insert(key.to_string(), Value::String(text.to_string()));
        }
    }
    Value::Object(normalized)
}

fn normalize_ocr_config(raw_value: &Value) -> Value {
    let provider = raw_value
        .get("provider")
        .map(|value| string_or_default(Some(value), "disabled").to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "disabled" | "tesseract" | "cloud_stub"))
        .unwrap_or_else(|| "disabled".to_string());
    let lang = raw_value
        .get("lang")
        .map(|value| string_or_default(Some(value), "chi_sim+eng"))
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 64
                && value
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '+' | '.' | '-'))
        })
        .unwrap_or_else(|| "chi_sim+eng".to_string());
    json!({
        "provider": provider,
        "lang": lang,
    })
}

fn sqlite_ref_to_json(value: rusqlite::types::ValueRef<'_>) -> Value {
    match value {
        rusqlite::types::ValueRef::Null => Value::Null,
        rusqlite::types::ValueRef::Integer(number) => Value::Number(Number::from(number)),
        rusqlite::types::ValueRef::Real(number) => {
            Number::from_f64(number).map_or(Value::Null, Value::Number)
        }
        rusqlite::types::ValueRef::Text(text) => {
            Value::String(String::from_utf8_lossy(text).to_string())
        }
        rusqlite::types::ValueRef::Blob(blob) => {
            Value::String(String::from_utf8_lossy(blob).to_string())
        }
    }
}

fn value_as_i64_or(value: Option<&Value>, default: i64) -> i64 {
    value.and_then(value_as_i64).unwrap_or(default)
}

fn count_rules_overview_learning_rules(
    connection: &Connection,
    user_id: i64,
) -> rusqlite::Result<i64> {
    connection.query_row(
        "SELECT COUNT(*) FROM import_learning_rules WHERE user_id = ?1",
        params![user_id],
        |row| row.get(0),
    )
}

fn list_rules_overview_learning_rules(
    connection: &Connection,
    user_id: i64,
) -> rusqlite::Result<Vec<Value>> {
    let mut statement = connection.prepare(
        "
        SELECT id, match_type, match_value, learned_type, learned_category_id,
               enabled, applied_count
        FROM import_learning_rules
        WHERE user_id = ?1
        ORDER BY updated_at DESC, id DESC
        LIMIT 500
        ",
    )?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok(json!({
            "id": row.get::<_, i64>("id")?,
            "matchType": row.get::<_, Option<String>>("match_type")?,
            "matchValue": row.get::<_, Option<String>>("match_value")?,
            "learnedType": row.get::<_, Option<String>>("learned_type")?,
            "learnedCategoryId": row.get::<_, Option<i64>>("learned_category_id")?,
            "enabled": row.get::<_, i64>("enabled")? != 0,
            "appliedCount": row.get::<_, Option<i64>>("applied_count")?.unwrap_or(0),
            "source": "learning",
        }))
    })?;
    rows.collect()
}

fn list_rules_overview_recurring_rules(
    connection: &Connection,
    user_id: i64,
) -> rusqlite::Result<Vec<Value>> {
    let mut statement = connection.prepare(
        "
        SELECT id, name, amount, frequency, enabled, next_date
        FROM recurring_bills
        WHERE user_id = ?1
        ORDER BY COALESCE(display_order, 0), name
        ",
    )?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok(json!({
            "id": row.get::<_, i64>("id")?,
            "name": row.get::<_, Option<String>>("name")?,
            "amount": row.get::<_, Option<f64>>("amount")?,
            "frequency": row.get::<_, Option<String>>("frequency")?,
            "enabled": row.get::<_, Option<i64>>("enabled")?.unwrap_or(1) != 0,
            "nextDate": row.get::<_, Option<String>>("next_date")?,
            "source": "recurring",
        }))
    })?;
    rows.collect()
}

fn category_rules_enabled_only(query: &CategoryRulesQuery) -> bool {
    !query
        .enabled_only
        .as_deref()
        .unwrap_or("true")
        .eq_ignore_ascii_case("false")
}

#[derive(Debug)]
struct LegacyCategoryEngineRuleRow {
    id: i64,
    category_id: i64,
    main_category: String,
    sub_category: String,
    category_type: i64,
    category_priority: i64,
    rule_priority: i64,
    rule_expression: String,
}

fn legacy_category_rules_setting_key(user_id: i64) -> String {
    format!("{LEGACY_CATEGORY_RULES_CONFIG_KEY_PREFIX}{user_id}")
}

fn list_legacy_category_engine_rules(
    connection: &Connection,
    user_id: i64,
) -> rusqlite::Result<Vec<Value>> {
    let mut statement = connection.prepare(
        "
        SELECT
            cr.id,
            cr.category_id,
            cr.priority AS rule_priority,
            cr.rule_expression,
            c.main_category,
            c.sub_category,
            c.type AS category_type,
            c.priority AS category_priority
        FROM category_rules cr
        JOIN categories c ON cr.category_id = c.id
        WHERE cr.user_id = ?1 AND c.user_id = ?1 AND cr.enabled = 1
        ORDER BY COALESCE(c.priority, cr.priority, 999999), cr.category_id, cr.id
        ",
    )?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok(LegacyCategoryEngineRuleRow {
            id: row.get("id")?,
            category_id: row.get("category_id")?,
            main_category: row
                .get::<_, Option<String>>("main_category")?
                .unwrap_or_default(),
            sub_category: row
                .get::<_, Option<String>>("sub_category")?
                .unwrap_or_default(),
            category_type: row.get::<_, Option<i64>>("category_type")?.unwrap_or(3),
            category_priority: row
                .get::<_, Option<i64>>("category_priority")?
                .unwrap_or(999_999),
            rule_priority: row.get::<_, Option<i64>>("rule_priority")?.unwrap_or(100),
            rule_expression: row
                .get::<_, Option<String>>("rule_expression")?
                .unwrap_or_default(),
        })
    })?;

    let mut rules = Vec::new();
    for (load_order, row) in rows.enumerate() {
        let row = row?;
        let Some(rule_type) = normalize_legacy_category_rule_type(row.category_type) else {
            continue;
        };
        if row.category_id <= 0
            || row.main_category.trim().is_empty()
            || row.sub_category.trim().is_empty()
        {
            continue;
        }
        rules.push(json!({
            "id": row.id,
            "category_id": row.category_id,
            "main": row.main_category.trim(),
            "sub": row.sub_category.trim(),
            "priority": row.category_priority,
            "category_priority": row.category_priority,
            "rule_priority": row.rule_priority,
            "keywords": row.rule_expression,
            "type": rule_type,
            "_load_order": load_order,
        }));
    }
    Ok(rules)
}

fn normalize_legacy_category_rule_type(raw_type: i64) -> Option<i64> {
    match raw_type {
        1 => Some(3),
        2..=5 => Some(raw_type),
        _ => None,
    }
}

