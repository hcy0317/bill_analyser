fn postgres_metadata_parent_id(metadata: &Value) -> i64 {
    match metadata.get("parent_id") {
        Some(Value::Number(number)) => number.as_i64().unwrap_or_default(),
        Some(Value::String(text)) => text.trim().parse::<i64>().unwrap_or_default(),
        Some(Value::Bool(flag)) => i64::from(*flag),
        Some(Value::Array(_) | Value::Object(_) | Value::Null) | None => 0,
    }
}

fn postgres_category_parts(path: Option<&str>, name: &str) -> (String, String) {
    let path = path.unwrap_or_default().trim();
    if let Some((main, sub)) = path.split_once('/') {
        return (main.trim().to_string(), sub.trim().to_string());
    }
    if !path.is_empty() {
        return (path.to_string(), String::new());
    }
    (name.trim().to_string(), String::new())
}

fn postgres_rule_expression_json(expression: &str, regex_enabled: bool) -> Value {
    json!({
        "expression": expression,
        "regex_enabled": regex_enabled,
    })
}

fn postgres_rule_expression_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Object(object) => object
            .get("expression")
            .or_else(|| object.get("rule_expression"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_default(),
        Value::Null => String::new(),
        Value::Number(_) | Value::Bool(_) | Value::Array(_) => value.to_string(),
    }
}

#[derive(Debug, Clone, Default)]
struct ExistingTemplates {
    by_key: BTreeMap<(i64, String), ExistingTemplate>,
}

impl ExistingTemplates {
    fn next_display_order(&self, template_type: i64) -> i64 {
        self.by_key
            .iter()
            .filter(|((existing_type, _), _)| *existing_type == template_type)
            .map(|(_, record)| record.display_order)
            .max()
            .unwrap_or_default()
            + 1
    }
}

#[derive(Debug, Clone)]
struct ExistingTemplate {
    id: i64,
    display_order: i64,
}

#[derive(Debug, Clone, PartialEq)]
struct SettingsTemplateImportValues {
    name: String,
    template_type: i64,
    description: Option<String>,
    transaction_type: Option<String>,
    category_id: Option<String>,
    source_account_id: String,
    destination_account_id: String,
    source_amount_minor_units: i64,
    destination_amount_minor_units: i64,
    hide_amount: bool,
    tag_ids: Value,
    comment: Option<String>,
    scheduled_frequency_type: Option<i64>,
    scheduled_frequency: Option<String>,
    scheduled_start_date: Option<String>,
    scheduled_end_date: Option<String>,
    scheduled_next_date: Option<String>,
    enabled: bool,
    auto_create: bool,
    display_order: i64,
    hidden: bool,
    utc_offset: i64,
}

fn settings_optional_text(value: Option<&Value>) -> Option<String> {
    let text = safe_text(value, "");
    (!text.is_empty()).then_some(text)
}

fn settings_zero_id_text(value: Option<&Value>) -> String {
    let text = safe_text(value, "0");
    if text.is_empty() {
        "0".to_string()
    } else {
        text
    }
}

fn settings_template_tag_ids(payload: &Value) -> Value {
    Value::Array(
        safe_text(payload.get("tag"), "")
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(|item| Value::String(item.to_string()))
            .collect(),
    )
}

fn settings_strict_minor_units(value: Option<&Value>) -> i64 {
    value.and_then(strict_minor_units).unwrap_or_default()
}

fn settings_i64_to_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or_else(|_| {
        if value.is_negative() {
            i32::MIN
        } else {
            i32::MAX
        }
    })
}
