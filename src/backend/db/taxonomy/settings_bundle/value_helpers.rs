// 中文导读：PostgreSQL settings bundle JSON 值 helper。
// 维护重点：只保留当前导入导出分区需要的解析与引用映射。

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
        Some(value) => json_to_display_string(value).trim().to_string(),
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

fn safe_minor_units(value: Option<&Value>, default: i64) -> i64 {
    value.and_then(strict_minor_units).unwrap_or(default)
}

fn strict_minor_units(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok())),
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                None
            } else {
                text.parse::<i64>().ok()
            }
        }
        Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => None,
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

fn json_to_display_string(value: &Value) -> String {
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
