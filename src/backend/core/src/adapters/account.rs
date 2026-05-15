use serde_json::Value;

pub fn parse_aliases_text(raw_value: &str) -> Vec<String> {
    let text = raw_value.trim();
    if text.is_empty() {
        return Vec::new();
    }

    if text.starts_with('[') {
        if let Ok(values) = serde_json::from_str::<Vec<Value>>(text) {
            return values
                .into_iter()
                .filter_map(|value| {
                    let alias = match value {
                        Value::String(text) => text,
                        other => other.to_string(),
                    };
                    let alias = alias.trim().to_string();
                    (!alias.is_empty()).then_some(alias)
                })
                .collect();
        }
    }

    text.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}
