// 中文导读：导入 stage2 的 JSON 与规则表达式投影辅助函数。
// 维护重点：保持纯转换/解析逻辑，不在本文件访问数据库或修改导入阶段顺序。

fn import_intelligence_category_parts(path: Option<&str>, name: &str) -> (String, String) {
    let path = path.unwrap_or_default().trim();
    if let Some((main, sub)) = path.split_once('/') {
        return (main.trim().to_string(), sub.trim().to_string());
    }
    if !path.is_empty() {
        return (path.to_string(), String::new());
    }
    (name.trim().to_string(), String::new())
}

fn import_intelligence_category_value(category: &ImportIntelligenceCategory) -> Value {
    json!({
        "id": category.id,
        "main": category.main_category,
        "sub": category.sub_category,
        "type": category.type_code,
    })
}

fn import_intelligence_account_value(account: &ImportIntelligenceAccount) -> Value {
    json!({
        "id": account.id,
        "name": account.name,
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
            .or_else(|| rule_expression_from_contains_any(object))
            .unwrap_or_default(),
        Value::Null => String::new(),
        Value::Number(_) | Value::Bool(_) | Value::Array(_) => value.to_string(),
    }
}

fn rule_expression_regex_enabled(value: &Value) -> bool {
    value
        .get("regex_enabled")
        .or_else(|| value.get("regexEnabled"))
        .map(json_value_truthy)
        .unwrap_or(false)
}

fn json_value_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_i64().unwrap_or_default() != 0,
        Value::String(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "y" | "on"
        ),
        Value::Null | Value::Array(_) | Value::Object(_) => false,
    }
}

fn rule_expression_from_contains_any(object: &Map<String, Value>) -> Option<String> {
    let operator = object
        .get("operator")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if operator != "contains_any" {
        return None;
    }
    let values = object
        .get("values")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(value_to_text)
        .map(|value| bill_analyser_core::category_rules::escape_rule_expression_term(&value))
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    (!values.is_empty()).then(|| format!("OR={{{}}}", values.join(",")))
}

fn value_to_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Array(_) | Value::Object(_) => Some(value.to_string()),
    }
}

fn text_from_json(value: &Value, key: &str) -> String {
    optional_text_from_json(value, key).unwrap_or_default()
}

fn optional_text_from_json(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(value_to_text)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn optional_i64_from_json(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str()?.trim().parse::<i64>().ok())
    })
}

fn string_map_from_json(value: Option<&Value>) -> BTreeMap<String, String> {
    value
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|object| object.iter())
        .filter_map(|(key, value)| {
            value_to_text(value).map(|value| (key.clone(), value.trim().to_ascii_lowercase()))
        })
        .filter(|(_, value)| !value.is_empty())
        .collect()
}


#[cfg(test)]
mod stage_json_helper_tests {
    use super::*;

    #[test]
    fn category_parts_and_values_preserve_current_projection() {
        assert_eq!(
            import_intelligence_category_parts(Some("餐饮 / 午餐"), "兜底"),
            ("餐饮".to_string(), "午餐".to_string()),
        );
        assert_eq!(
            import_intelligence_category_parts(Some("投资"), "兜底"),
            ("投资".to_string(), String::new()),
        );
        assert_eq!(
            import_intelligence_category_parts(None, "未分类"),
            ("未分类".to_string(), String::new()),
        );

        let category = ImportIntelligenceCategory {
            id: 7,
            main_category: "餐饮".to_string(),
            sub_category: "午餐".to_string(),
            type_code: 1,
        };
        assert_eq!(import_intelligence_category_value(&category)["main"], "餐饮");
        assert_eq!(import_intelligence_category_value(&category)["type"], 1);

        let account = ImportIntelligenceAccount {
            id: 42,
            name: "招商工资卡".to_string(),
        };
        assert_eq!(import_intelligence_account_value(&account)["name"], "招商工资卡");
    }

    #[test]
    fn rule_expression_helpers_normalize_current_json_shapes() {
        assert_eq!(rule_expression_string(&json!("星巴克")), "星巴克");
        assert_eq!(
            rule_expression_string(&json!({ "rule_expression": "counterparty*=咖啡" })),
            "counterparty*=咖啡",
        );
        assert_eq!(
            rule_expression_string(&json!({
                "operator": "contains_any",
                "values": ["a,b", "", "午餐"]
            })),
            "OR={a\\,b,午餐}",
        );
        assert_eq!(rule_expression_string(&Value::Null), "");
        assert_eq!(rule_expression_string(&json!(12)), "12");
        assert!(rule_expression_regex_enabled(&json!({ "regex_enabled": "yes" })));
        assert!(rule_expression_regex_enabled(&json!({ "regexEnabled": 1 })));
        assert!(!rule_expression_regex_enabled(&json!({ "regex_enabled": "off" })));
        assert!(json_value_truthy(&json!(true)));
        assert!(json_value_truthy(&json!(2)));
        assert!(json_value_truthy(&json!("ON")));
        assert!(!json_value_truthy(&json!({ "object": true })));
        assert_eq!(rule_expression_from_contains_any(json!({ "operator": "eq" }).as_object().unwrap()), None);
        assert_eq!(rule_expression_from_contains_any(json!({ "operator": "contains_any", "values": [] }).as_object().unwrap()), None);
    }

    #[test]
    fn scalar_json_helpers_trim_parse_and_lowercase_maps() {
        let value = json!({
            "name": "  招商卡  ",
            "id": "42",
            "bad_id": "x",
            "metadata": {
                "Payment": " WeChat ",
                "empty": " "
            }
        });
        assert_eq!(text_from_json(&value, "name"), "招商卡");
        assert_eq!(optional_text_from_json(&value, "missing"), None);
        assert_eq!(optional_i64_from_json(&value, "id"), Some(42));
        assert_eq!(optional_i64_from_json(&json!({ "id": 7 }), "id"), Some(7));
        assert_eq!(optional_i64_from_json(&value, "bad_id"), None);
        assert_eq!(value_to_text(&Value::Null), None);
        assert_eq!(value_to_text(&json!(" raw ")), Some(" raw ".to_string()));
        assert_eq!(value_to_text(&json!(12)), Some("12".to_string()));
        assert_eq!(value_to_text(&json!(true)), Some("true".to_string()));
        assert_eq!(value_to_text(&json!(["a"])), Some("[\"a\"]".to_string()));
        assert_eq!(
            value_to_text(&json!({ "key": "value" })),
            Some("{\"key\":\"value\"}".to_string())
        );

        let map = string_map_from_json(value.get("metadata"));
        assert_eq!(map.get("Payment").map(String::as_str), Some("wechat"));
        assert!(!map.contains_key("empty"));
        assert!(string_map_from_json(None).is_empty());
    }
}
