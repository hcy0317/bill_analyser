// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

fn has_template_ref_value(item: &Value, keys: &[&str]) -> bool {
    keys.iter().any(|key| {
        let value = safe_text(get_any(item, &[*key]), "");
        if value.is_empty() {
            return false;
        }
        if is_optional_zero_id_key(key) && value == "0" {
            return false;
        }
        true
    })
}

fn is_optional_zero_id_key(key: &str) -> bool {
    matches!(
        key,
        "sourceAccountId"
            | "source_account_id"
            | "destinationAccountId"
            | "destination_account_id"
            | "categoryId"
            | "category_id"
    )
}

fn id_ref_map(items: &[Value], prefix: &str) -> BTreeMap<i64, String> {
    items
        .iter()
        .filter_map(|item| {
            let id = safe_int(item.get("id"), 0);
            (id > 0).then(|| (id, format!("{prefix}:{id}")))
        })
        .collect()
}

fn id_name_map(items: &[Value], name_key: &str) -> BTreeMap<i64, String> {
    items
        .iter()
        .filter_map(|item| {
            let id = safe_int(item.get("id"), 0);
            (id > 0).then(|| (id, safe_text(item.get(name_key), "")))
        })
        .collect()
}

fn category_name_map(items: &[Value]) -> BTreeMap<i64, String> {
    items
        .iter()
        .filter_map(|item| {
            let id = safe_int(item.get("id"), 0);
            let name = settings_category_name(item);
            (id > 0).then_some((id, name))
        })
        .collect()
}

fn settings_category_name(category: &Value) -> String {
    [
        safe_text(category.get("main_category"), ""),
        safe_text(category.get("sub_category"), ""),
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join("/")
}

fn get_any<'payload>(data: &'payload Value, keys: &[&str]) -> Option<&'payload Value> {
    let object = data.as_object()?;
    keys.iter().find_map(|key| object.get(*key))
}

fn get_any_with_default<'payload>(
    data: &'payload Value,
    keys: &[&str],
    default: Option<&'payload Value>,
) -> Option<&'payload Value> {
    get_any(data, keys).or(default)
}

fn scheduled_start_value(item: &Value) -> Option<&Value> {
    get_any(item, &["scheduledStartDate", "startDate"])
}

fn safe_text(value: Option<&Value>, default: &str) -> String {
    match value {
        None | Some(Value::Null) => default.to_string(),
        Some(Value::String(text)) => text.trim().to_string(),
        Some(value) => json_to_python_like_string(value).trim().to_string(),
    }
}

fn safe_int(value: Option<&Value>, default: i64) -> i64 {
    match value {
        Some(Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| number.as_f64().map(|value| value as i64))
            .unwrap_or(default),
        Some(Value::String(text)) => text.trim().parse::<i64>().unwrap_or(default),
        Some(Value::Bool(flag)) => i64::from(*flag),
        _ => default,
    }
}

fn safe_float(value: Option<&Value>, default: f64) -> f64 {
    match value {
        Some(Value::Number(number)) => number.as_f64().unwrap_or(default),
        Some(Value::String(text)) => text.trim().parse::<f64>().unwrap_or(default),
        Some(Value::Bool(flag)) => {
            if *flag {
                1.0
            } else {
                0.0
            }
        }
        _ => default,
    }
}

fn safe_bool(value: Option<&Value>) -> bool {
    match value {
        Some(Value::String(text)) => matches!(
            text.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Some(Value::Bool(flag)) => *flag,
        Some(Value::Number(number)) => number.as_i64().unwrap_or(0) != 0,
        Some(Value::Array(values)) => !values.is_empty(),
        Some(Value::Object(values)) => !values.is_empty(),
        _ => false,
    }
}

fn safe_bool_with_default(value: Option<&Value>, default: bool) -> bool {
    match value {
        None | Some(Value::Null) => default,
        Some(_) => safe_bool(value),
    }
}

fn map_int(map_value: &Value, key: &str) -> i64 {
    map_get_int(map_value, key).unwrap_or(0)
}

fn map_get_int(map_value: &Value, key: &str) -> Option<i64> {
    if key.is_empty() {
        return None;
    }
    map_value
        .as_object()?
        .get(key)
        .map(|value| safe_int(Some(value), 0))
}

fn local_id_ref(prefix: &str, item_id: i64) -> String {
    format!("{LOCAL_REF_NAMESPACE}:{prefix}:{item_id}")
}

fn split_category_name(value: Option<&Value>) -> (String, String) {
    let text = safe_text(value, "");
    if text.is_empty() {
        return (String::new(), String::new());
    }
    if let Some((main, sub)) = text.split_once('/') {
        return (main.trim().to_string(), sub.trim().to_string());
    }
    (text, String::new())
}

fn json_to_python_like_string(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(flag) => {
            if *flag {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn external_ref(item: &Value, prefix: &str) -> String {
    let explicit = safe_text(get_any(item, &["externalRef", "external_ref"]), "");
    if !explicit.is_empty() {
        if explicit.starts_with(&format!("{LOCAL_REF_NAMESPACE}:")) {
            return String::new();
        }
        return explicit;
    }
    let item_id = safe_text(get_any(item, &["id", "sourceId", "source_id"]), "");
    if item_id.is_empty() {
        String::new()
    } else {
        format!("{prefix}:{item_id}")
    }
}

fn category_name(main: &str, sub: &str) -> String {
    [main, sub]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

fn sql_value_or_null(value: Option<&Value>) -> DbResult<SqlValue> {
    Ok(match value {
        Some(Value::Null) | None => SqlValue::Null,
        Some(Value::String(text)) => SqlValue::Text(text.clone()),
        Some(Value::Bool(flag)) => SqlValue::Integer(i64::from(*flag)),
        Some(Value::Number(number)) => {
            if let Some(value) = number.as_i64() {
                SqlValue::Integer(value)
            } else if let Some(value) = number.as_u64().and_then(|value| i64::try_from(value).ok())
            {
                SqlValue::Integer(value)
            } else if let Some(value) = number.as_f64() {
                SqlValue::Real(value)
            } else {
                SqlValue::Null
            }
        }
        Some(value @ (Value::Array(_) | Value::Object(_))) => SqlValue::Text(
            serde_json::to_string(value)
                .map_err(|error| DbError::InvalidOperation(error.to_string()))?,
        ),
    })
}

fn dump_json_object(value: Option<&Value>) -> String {
    let loaded = match value {
        Some(Value::String(text)) => {
            serde_json::from_str::<Value>(text).unwrap_or_else(|_| json!({}))
        }
        Some(Value::Object(object)) => Value::Object(object.clone()),
        _ => json!({}),
    };
    let object = loaded
        .as_object()
        .cloned()
        .map(Value::Object)
        .unwrap_or_else(|| json!({}));
    serde_json::to_string(&object).unwrap_or_else(|_| "{}".to_string())
}

fn is_masked_secret(value: &str) -> bool {
    matches!(value.trim(), "" | "********" | "redacted" | "<redacted>")
}

fn normalize_ocr_config(raw_value: &Value) -> Value {
    let normalized = bill_analyser_core::normalize_ocr_config(Some(raw_value));
    json!({
        "provider": normalized.provider,
        "lang": normalized.lang,
        "model": normalized.model,
        "base_url": normalized.base_url,
        "parameters": normalized.parameters,
        "credential_config": normalized.credential_config,
    })
}

fn table_has_column(
    transaction: &Transaction<'_>,
    table_name: &str,
    column_name: &str,
) -> DbResult<bool> {
    let mut statement = transaction.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        if row? == column_name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn utc_now_iso() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_bundle_normalizes_sections_and_rejects_bad_schema() {
        let normalized = normalize_settings_bundle_sections(&json!({
            "schemaVersion": 1,
            "sections": {
                "accounts": [{"name": "Cash"}, "ignored", null],
                "transactionTags": null
            }
        }))
        .unwrap();

        assert_eq!(normalized["accounts"].as_array().unwrap().len(), 1);
        assert_eq!(normalized["transactionTags"].as_array().unwrap().len(), 0);
        assert_eq!(normalized["ocrConfig"].as_array().unwrap().len(), 0);

        let error = normalize_settings_bundle_sections(&json!({
            "schemaVersion": true,
            "sections": {}
        }))
        .unwrap_err()
        .to_string();
        assert!(error.contains("Unsupported settings bundle schemaVersion"));
    }

    #[test]
    fn settings_bundle_exports_taxonomy_sections_with_refs() {
        let exported = export_taxonomy_sections(&json!({
            "accounts": [{
                "id": 10,
                "name": "Wallet",
                "type": 1,
                "currency": "CNY",
                "balance": 12.5,
                "initial_balance": 2.5
            }],
            "categories": [{
                "id": 20,
                "type": 3,
                "main_category": "Food",
                "sub_category": "Coffee"
            }],
            "tags": [{"id": 30, "name": "Work"}],
            "templates": [{
                "id": "40",
                "templateType": 1,
                "name": "Latte",
                "categoryId": "20",
                "sourceAccountId": "10",
                "tagIds": ["30"]
            }],
            "scheduled": []
        }))
        .unwrap();

        assert_eq!(exported["accounts"][0]["externalRef"], "account:10");
        assert_eq!(
            exported["transactionCategories"][0]["externalRef"],
            "category:20"
        );
        assert_eq!(
            exported["transactionTemplates"][0]["categoryName"],
            "Food/Coffee"
        );
        assert_eq!(
            exported["transactionTemplates"][0]["tagRefs"],
            json!(["tag:30"])
        );
    }

    #[test]
    fn settings_bundle_resolves_template_refs_without_external_id_collision() {
        let resolved = resolve_template_payload(&json!({
            "item": {
                "name": "T",
                "categoryRef": "category:123",
                "sourceAccountRef": "account:1",
                "tagRefs": ["tag:1"],
                "sourceAmount": 9.5
            },
            "category_ref_map": {
                "__local_settings_bundle_id__:category:20": 200
            },
            "account_ref_map": {
                "__local_settings_bundle_id__:account:10": 100
            },
            "tag_ref_map": {
                "__local_settings_bundle_id__:tag:30": 300
            }
        }));

        assert!(resolved["unresolved"].as_bool().unwrap());
        assert_eq!(resolved["warnings"][0], "Template tag ref not found: tag:1");

        let legacy_id_resolved = resolve_template_payload(&json!({
            "item": {
                "name": "T2",
                "categoryId": 20,
                "sourceAccountId": 10,
                "tagIds": [30],
                "sourceAmount": 9.5
            },
            "category_ref_map": {
                "__local_settings_bundle_id__:category:20": 200
            },
            "account_ref_map": {
                "__local_settings_bundle_id__:account:10": 100
            },
            "tag_ref_map": {
                "__local_settings_bundle_id__:tag:30": 300
            }
        }));
        assert!(!legacy_id_resolved["unresolved"].as_bool().unwrap());
        assert_eq!(legacy_id_resolved["payload"]["category"], "200");
        assert_eq!(legacy_id_resolved["payload"]["account"], "100");
        assert_eq!(legacy_id_resolved["payload"]["tag"], "300");
    }

    #[test]
    fn settings_bundle_normalizes_import_values() {
        let account = normalize_account_import(&json!({
            "item": {
                "name": "Card",
                "parentRef": "account:root",
                "hidden": "yes"
            },
            "ref_map": {"account:root": 7}
        }));
        assert_eq!(account["parent_id"], 7);
        assert_eq!(account["hidden"], 1);

        let category = normalize_category_import(&json!({
            "item": {"mainCategory": "Food", "hidden": true}
        }));
        assert_eq!(category["main_category"], "Food");
        assert_eq!(category["type"], 3);

        let tag = normalize_tag_import(&json!({
            "item": {"name": "Work", "displayOrder": "4"}
        }));
        assert_eq!(tag["display_order"], 4);
    }

    #[test]
    fn settings_bundle_import_persists_account_rules_after_account_refs() {
        let mut connection = Connection::open_in_memory().expect("open");
        crate::schema::init_foundational_schema(&connection).expect("schema");
        crate::recurring::init_recurring_runtime_schema(&connection).expect("templates schema");
        connection
            .execute(
                "INSERT INTO users(id, username, email, password_hash, created_at, updated_at)
                 VALUES (42, 'settings-user', 'settings@example.test', 'hash', 'now', 'now')",
                [],
            )
            .expect("user");

        let imported = import_settings_bundle(
            &mut connection,
            &json!({
                "schemaVersion": 1,
                "sections": {
                    "accounts": [{
                        "externalRef": "account:cash",
                        "name": "现金账户",
                        "type": 1,
                        "currency": "CNY",
                        "balance": 0
                    }],
                    "accountRecognitionRules": [{
                        "accountRef": "account:cash",
                        "name": "现金账户规则",
                        "priority": 1,
                        "ruleExpression": "OR={现金}",
                        "fieldScope": ["counterparty"],
                        "accountRoleScope": "source",
                        "transactionTypeScope": "expense"
                    }]
                }
            }),
            42,
            false,
        )
        .expect("import bundle");

        assert_eq!(
            imported["sections"]["accountRecognitionRules"]["created"],
            1
        );
        let count = connection
            .query_row(
                "SELECT COUNT(*) FROM account_rules WHERE user_id = 42 AND rule_expression = 'OR={现金}'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("account rule count");
        assert_eq!(count, 1);
    }
}
